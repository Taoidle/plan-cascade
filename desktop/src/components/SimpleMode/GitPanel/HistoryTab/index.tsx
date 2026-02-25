/**
 * HistoryTab Component
 *
 * Main container for the commit history graph view.
 * Layout: search bar at top, CommitGraph in middle, CommitDetail at bottom (expandable).
 * Supports search filtering, branch filtering, and infinite scroll pagination.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';
import { useSettingsStore } from '../../../../store/settings';
import { useGitGraph } from '../../../../hooks/useGitGraph';
import CommitGraph from './CommitGraph';
import CommitDetail from './CommitDetail';
import ContextMenu, { type ContextMenuState } from './ContextMenu';

// ---------------------------------------------------------------------------
// HistoryTab Component
// ---------------------------------------------------------------------------

export function HistoryTab() {
  const { t } = useTranslation('git');
  const repoPath = useSettingsStore((s) => s.workspacePath);
  // Git graph data
  const { graphLayout, commits, branches, isLoading, error, loadMore, refresh, hasMore } = useGitGraph({ repoPath });

  // Git store state
  const {
    selectedCommitSha,
    compareSelection,
    commitDetailExpanded,
    branchFilter,
    searchQuery,
    selectedCommitDiff,
    setSelectedCommitSha,
    setCompareSelection,
    setCommitDetailExpanded,
    setBranchFilter,
    setSearchQuery,
    setSelectedCommitDiff,
  } = useGitStore();

  // Local state
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // ---------------------------------------------------------------------------
  // Derived state
  // ---------------------------------------------------------------------------

  /** Find the HEAD commit SHA from branch info */
  const headSha = useMemo(() => {
    const headBranch = branches.find((b) => b.is_head);
    return headBranch?.tip_sha ?? (commits.length > 0 ? commits[0].sha : null);
  }, [branches, commits]);

  /** Filter commits by search query and branch */
  const filteredCommits = useMemo(() => {
    let filtered = commits;

    // Filter by search query
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (c) =>
          c.message.toLowerCase().includes(q) ||
          c.author_name.toLowerCase().includes(q) ||
          c.author_email.toLowerCase().includes(q) ||
          c.short_sha.toLowerCase().includes(q) ||
          c.sha.toLowerCase().startsWith(q),
      );
    }

    return filtered;
  }, [commits, searchQuery]);

  /** Build a filtered graph layout that only includes filtered commits */
  const filteredGraphLayout = useMemo(() => {
    if (!graphLayout) return null;
    if (!searchQuery.trim() && !branchFilter) return graphLayout;

    const filteredShas = new Set(filteredCommits.map((c) => c.sha));

    // When filtering, we re-index rows to be sequential
    const filteredNodes = graphLayout.nodes.filter((n) => filteredShas.has(n.sha)).map((n, i) => ({ ...n, row: i }));

    const filteredEdges = graphLayout.edges.filter((e) => filteredShas.has(e.from_sha) && filteredShas.has(e.to_sha));

    // Remap edge rows to match filtered node rows
    const shaToRow = new Map(filteredNodes.map((n) => [n.sha, n.row]));
    const remappedEdges = filteredEdges.map((e) => ({
      ...e,
      from_row: shaToRow.get(e.from_sha) ?? e.from_row,
      to_row: shaToRow.get(e.to_sha) ?? e.to_row,
    }));

    return {
      nodes: filteredNodes,
      edges: remappedEdges,
      max_lane: graphLayout.max_lane,
    };
  }, [graphLayout, filteredCommits, searchQuery, branchFilter]);

  // ---------------------------------------------------------------------------
  // Commit selection
  // ---------------------------------------------------------------------------

  const handleSelectCommit = useCallback(
    async (sha: string) => {
      setSelectedCommitSha(sha);
      setSelectedCommitDiff(null);

      if (!repoPath) return;

      // Fetch diff for the selected commit via git show --numstat
      try {
        const { Command } = await import('@tauri-apps/plugin-shell');
        const commit = commits.find((c) => c.sha === sha);
        if (!commit) return;

        // Use git show --numstat to get file stats
        const cmd = Command.create('git', ['show', '--numstat', '--format=', sha], {
          cwd: repoPath,
        });
        const output = await cmd.execute();

        if (output.code === 0 && output.stdout) {
          let totalAdditions = 0;
          let totalDeletions = 0;

          // Parse numstat: "added\tdeleted\tpath"
          const files = output.stdout
            .trim()
            .split('\n')
            .filter((line: string) => line.trim())
            .map((line: string) => {
              const parts = line.split('\t');
              const additions = parseInt(parts[0], 10) || 0;
              const deletions = parseInt(parts[1], 10) || 0;
              const path = parts[2] || '';
              totalAdditions += additions;
              totalDeletions += deletions;

              // Create synthetic hunks with proper line counts for stats display
              const lines: { kind: 'addition' | 'deletion' | 'context'; content: string }[] = [];
              for (let i = 0; i < additions; i++) lines.push({ kind: 'addition', content: '' });
              for (let i = 0; i < deletions; i++) lines.push({ kind: 'deletion', content: '' });

              return {
                path,
                is_new: additions > 0 && deletions === 0,
                is_deleted: deletions > 0 && additions === 0,
                is_renamed: false,
                hunks:
                  lines.length > 0
                    ? [{ header: '', old_start: 1, old_count: deletions, new_start: 1, new_count: additions, lines }]
                    : [],
              };
            });

          setSelectedCommitDiff({
            files,
            total_additions: totalAdditions,
            total_deletions: totalDeletions,
          });
        }
      } catch {
        // Silently fail - diff is optional enhancement
      }
    },
    [repoPath, commits, setSelectedCommitSha, setSelectedCommitDiff],
  );

  const handleCompareCommit = useCallback(
    (sha: string) => {
      if (!selectedCommitSha || sha === selectedCommitSha) return;
      setCompareSelection({
        baseSha: selectedCommitSha,
        compareSha: sha,
      });
    },
    [selectedCommitSha, setCompareSelection],
  );

  // ---------------------------------------------------------------------------
  // Context menu
  // ---------------------------------------------------------------------------

  const handleContextMenu = useCallback((sha: string, event: React.MouseEvent) => {
    setContextMenu({
      sha,
      x: event.clientX,
      y: event.clientY,
    });
  }, []);

  const closeContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  // Close context menu on click outside
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener('click', handler);
    return () => document.removeEventListener('click', handler);
  }, [contextMenu]);

  // ---------------------------------------------------------------------------
  // Keyboard shortcut: Ctrl/Cmd+F to focus search
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);

  // ---------------------------------------------------------------------------
  // Infinite scroll
  // ---------------------------------------------------------------------------

  const handleScrollToBottom = useCallback(() => {
    if (hasMore && !isLoading) {
      loadMore();
    }
  }, [hasMore, isLoading, loadMore]);

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Search & filter bar */}
      <div className="shrink-0 flex items-center gap-2 px-3 py-2 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        {/* Search input */}
        <div className="flex-1 relative">
          <svg
            className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
          </svg>
          <input
            ref={searchInputRef}
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t('historyTab.searchPlaceholder')}
            className={clsx(
              'w-full pl-7 pr-2 py-1.5 text-xs rounded-md',
              'bg-white dark:bg-gray-900',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-800 dark:text-gray-200',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:outline-none focus:ring-1 focus:ring-blue-500 dark:focus:ring-blue-400',
              'transition-colors',
            )}
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>

        {/* Branch filter dropdown */}
        <select
          value={branchFilter ?? ''}
          onChange={(e) => setBranchFilter(e.target.value || null)}
          className={clsx(
            'text-xs px-2 py-1.5 rounded-md',
            'bg-white dark:bg-gray-900',
            'border border-gray-200 dark:border-gray-700',
            'text-gray-700 dark:text-gray-300',
            'focus:outline-none focus:ring-1 focus:ring-blue-500',
            'transition-colors',
          )}
        >
          <option value="">{t('historyTab.allBranches')}</option>
          {branches.map((branch) => (
            <option key={branch.name} value={branch.name}>
              {branch.name}
              {branch.is_head ? ` ${t('historyTab.head')}` : ''}
            </option>
          ))}
        </select>

        {/* Refresh button */}
        <button
          onClick={refresh}
          disabled={isLoading}
          className={clsx(
            'p-1.5 rounded-md text-gray-500 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors',
          )}
          title={t('historyTab.refresh')}
        >
          <svg
            className={clsx('w-3.5 h-3.5', isLoading && 'animate-spin')}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
            />
          </svg>
        </button>
      </div>

      {/* Compare mode indicator */}
      {compareSelection && (
        <div className="shrink-0 flex items-center justify-between px-3 py-1.5 bg-purple-50 dark:bg-purple-900/20 border-b border-purple-200 dark:border-purple-800">
          <span className="text-xs text-purple-700 dark:text-purple-300">
            {t('historyTab.comparing', {
              base: compareSelection.baseSha.slice(0, 7),
              compare: compareSelection.compareSha.slice(0, 7),
            })}
          </span>
          <button
            onClick={() => setCompareSelection(null)}
            className="text-xs text-purple-600 dark:text-purple-400 hover:text-purple-800 dark:hover:text-purple-200"
          >
            {t('historyTab.clear')}
          </button>
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="shrink-0 px-3 py-2 bg-red-50 dark:bg-red-900/20 border-b border-red-200 dark:border-red-800">
          <p className="text-xs text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}

      {/* Loading state (initial) */}
      {isLoading && !graphLayout && (
        <div className="flex-1 flex items-center justify-center">
          <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
            <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full" />
            {t('historyTab.loadingHistory')}
          </div>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !error && filteredGraphLayout && filteredGraphLayout.nodes.length === 0 && (
        <div className="flex-1 flex items-center justify-center">
          <div className="text-center py-8">
            <svg
              className="w-10 h-10 mx-auto text-gray-300 dark:text-gray-600 mb-2"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={1.5}
                d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {searchQuery ? t('historyTab.noMatchingCommits') : t('historyTab.noCommitHistory')}
            </p>
          </div>
        </div>
      )}

      {/* Commit graph */}
      {filteredGraphLayout && filteredGraphLayout.nodes.length > 0 && (
        <CommitGraph
          graphLayout={filteredGraphLayout}
          commits={filteredCommits}
          selectedCommitSha={selectedCommitSha}
          compareCommitSha={compareSelection?.compareSha ?? null}
          onSelectCommit={handleSelectCommit}
          onCompareCommit={handleCompareCommit}
          onContextMenu={handleContextMenu}
          onScrollToBottom={handleScrollToBottom}
          headSha={headSha}
          searchQuery={searchQuery}
        />
      )}

      {/* Loading more indicator */}
      {isLoading && graphLayout && (
        <div className="shrink-0 flex items-center justify-center py-2 border-t border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
            <div className="animate-spin h-3 w-3 border border-gray-400 border-t-transparent rounded-full" />
            {t('historyTab.loadingMore')}
          </div>
        </div>
      )}

      {/* Commit detail panel */}
      {selectedCommitSha && commitDetailExpanded && (
        <CommitDetail
          commit={commits.find((c) => c.sha === selectedCommitSha) ?? null}
          diff={selectedCommitDiff}
          repoPath={repoPath}
          onClose={() => setCommitDetailExpanded(false)}
        />
      )}

      {/* Context menu */}
      {contextMenu && (
        <ContextMenu
          state={contextMenu}
          repoPath={repoPath}
          commit={commits.find((c) => c.sha === contextMenu.sha) ?? null}
          onClose={closeContextMenu}
          onRefresh={refresh}
        />
      )}
    </div>
  );
}

export default HistoryTab;
