/**
 * ChangesTab Component
 *
 * Main Changes view with CommitBox at top, then MergeBar (conditional),
 * StashBar, and StagingArea sections below.
 *
 * Feature-002, Story-004 + Story-005
 */

import { useGitStore } from '../../../../store/git';
import { CommitBox } from './CommitBox';
import { MergeBar } from './MergeBar';
import { StashBar } from './StashBar';
import { StagingArea } from './StagingArea';

export function ChangesTab() {
  const error = useGitStore((s) => s.error);

  return (
    <div className="flex flex-col">
      {/* Error banner */}
      {error && (
        <div className="px-3 py-2 bg-red-50 dark:bg-red-900/20 border-b border-red-200 dark:border-red-800">
          <div className="flex items-center justify-between">
            <p className="text-xs text-red-600 dark:text-red-400 flex-1 mr-2">{error}</p>
            <button
              onClick={() => useGitStore.getState().setError(null)}
              className="shrink-0 text-red-400 hover:text-red-600 dark:hover:text-red-300 transition-colors"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>
      )}

      {/* Merge/Rebase bar (conditional) */}
      <MergeBar />

      {/* Commit box */}
      <CommitBox />

      {/* Stash bar */}
      <StashBar />

      {/* Staging area */}
      <StagingArea />
    </div>
  );
}

export default ChangesTab;
