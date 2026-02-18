/**
 * CommitBox Component
 *
 * Commit message textarea with conventional commit placeholder,
 * commit button, amend checkbox, and a reserved AI generate button area.
 *
 * Feature-002, Story-005
 */

import { useCallback } from 'react';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';

export function CommitBox() {
  const commitMessage = useGitStore((s) => s.commitMessage);
  const setCommitMessage = useGitStore((s) => s.setCommitMessage);
  const isAmend = useGitStore((s) => s.isAmend);
  const setIsAmend = useGitStore((s) => s.setIsAmend);
  const commit = useGitStore((s) => s.commit);
  const status = useGitStore((s) => s.status);
  const isLoading = useGitStore((s) => s.isLoading);

  const stagedCount = status?.staged.length ?? 0;
  const canCommit = stagedCount > 0 && commitMessage.trim().length > 0 && !isLoading;

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

  return (
    <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
      {/* Message input */}
      <textarea
        value={commitMessage}
        onChange={(e) => setCommitMessage(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="feat: describe your changes (Cmd+Enter to commit)"
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
            <span className="text-2xs text-gray-500 dark:text-gray-400">Amend</span>
          </label>

          {/* AI Generate button placeholder (feature-005) */}
          <button
            disabled
            className="text-2xs px-2 py-1 rounded text-gray-400 dark:text-gray-500 border border-dashed border-gray-300 dark:border-gray-600 opacity-50 cursor-not-allowed"
            title="AI-generated commit message (coming soon)"
          >
            <span className="flex items-center gap-1">
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
              AI
            </span>
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
          {isAmend ? 'Amend' : 'Commit'}
          {stagedCount > 0 && (
            <span className="ml-1 opacity-75">({stagedCount})</span>
          )}
        </button>
      </div>
    </div>
  );
}

export default CommitBox;
