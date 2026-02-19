/**
 * useGitGraph Hook
 *
 * Fetches commit graph data from the Rust backend via Tauri IPC,
 * caches results in local state, and supports pagination and
 * auto-refresh on git events.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { CommandResponse } from '../lib/tauri';
import type {
  CommitNode,
  GraphLayout,
  BranchInfo,
} from '../types/git';
import { useGitStore } from '../store/git';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Default number of commits to fetch per page */
const DEFAULT_PAGE_SIZE = 200;

/** Minimum interval between auto-refreshes (ms) */
const REFRESH_DEBOUNCE_MS = 1000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface UseGitGraphOptions {
  /** Repository path to fetch data from */
  repoPath: string | null;
  /** Number of commits to fetch per page */
  pageSize?: number;
  /** Whether to auto-refresh on git events */
  autoRefresh?: boolean;
}

interface UseGitGraphReturn {
  /** Graph layout data (nodes, edges, max_lane) */
  graphLayout: GraphLayout | null;
  /** Flat list of commits matching the current graph */
  commits: CommitNode[];
  /** List of branches */
  branches: BranchInfo[];
  /** Whether data is currently loading */
  isLoading: boolean;
  /** Error message if fetch failed */
  error: string | null;
  /** Load more commits (pagination) */
  loadMore: () => Promise<void>;
  /** Manually refresh the graph data */
  refresh: () => Promise<void>;
  /** Whether there are potentially more commits to load */
  hasMore: boolean;
  /** Total commits currently loaded */
  totalLoaded: number;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useGitGraph({
  repoPath,
  pageSize = DEFAULT_PAGE_SIZE,
  autoRefresh = true,
}: UseGitGraphOptions): UseGitGraphReturn {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(true);
  const [currentCount, setCurrentCount] = useState(pageSize);

  // Use the git store for cached data
  const {
    commitLog: commits,
    graphLayout,
    branches,
    setCommits,
    setGraphLayout,
    setBranches,
  } = useGitStore();

  // Track last refresh time for debouncing
  const lastRefreshRef = useRef<number>(0);
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ---------------------------------------------------------------------------
  // Fetch graph data
  // ---------------------------------------------------------------------------

  const fetchGraphData = useCallback(
    async (count: number) => {
      if (!repoPath) {
        setError('No repository path specified');
        return;
      }

      setIsLoading(true);
      setError(null);

      try {
        // Fetch commits and graph layout in parallel
        const [logResult, graphResult, branchResult] = await Promise.all([
          invoke<CommandResponse<CommitNode[]>>('git_log', {
            repoPath,
            count,
            allBranches: true,
          }),
          invoke<CommandResponse<GraphLayout>>('git_log_graph', {
            repoPath,
            count,
          }),
          invoke<CommandResponse<BranchInfo[]>>('git_list_branches', {
            repoPath,
          }),
        ]);

        // Process commits
        if (logResult.success && logResult.data) {
          setCommits(logResult.data);
          // If we got fewer commits than requested, there are no more
          setHasMore(logResult.data.length >= count);
        } else {
          setError(logResult.error || 'Failed to fetch commit log');
          setCommits([]);
        }

        // Process graph layout
        if (graphResult.success && graphResult.data) {
          setGraphLayout(graphResult.data);
        } else {
          setGraphLayout(null);
        }

        // Process branches
        if (branchResult.success && branchResult.data) {
          setBranches(branchResult.data);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        setCommits([]);
        setGraphLayout(null);
      } finally {
        setIsLoading(false);
        lastRefreshRef.current = Date.now();
      }
    },
    [repoPath, setCommits, setGraphLayout, setBranches]
  );

  // ---------------------------------------------------------------------------
  // Refresh
  // ---------------------------------------------------------------------------

  const refresh = useCallback(async () => {
    setCurrentCount(pageSize);
    await fetchGraphData(pageSize);
  }, [pageSize, fetchGraphData]);

  // ---------------------------------------------------------------------------
  // Load more (pagination)
  // ---------------------------------------------------------------------------

  const loadMore = useCallback(async () => {
    if (!hasMore || isLoading) return;
    const nextCount = currentCount + pageSize;
    setCurrentCount(nextCount);
    await fetchGraphData(nextCount);
  }, [hasMore, isLoading, currentCount, pageSize, fetchGraphData]);

  // ---------------------------------------------------------------------------
  // Initial fetch
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (repoPath) {
      fetchGraphData(currentCount);
    }
    // Only refetch when repoPath changes, not currentCount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath]);

  // ---------------------------------------------------------------------------
  // Auto-refresh on git events
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (!autoRefresh || !repoPath) return;

    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen('git-status-changed', () => {
          // Debounce rapid events
          const now = Date.now();
          const elapsed = now - lastRefreshRef.current;

          if (refreshTimerRef.current) {
            clearTimeout(refreshTimerRef.current);
          }

          if (elapsed >= REFRESH_DEBOUNCE_MS) {
            fetchGraphData(currentCount);
          } else {
            refreshTimerRef.current = setTimeout(() => {
              fetchGraphData(currentCount);
            }, REFRESH_DEBOUNCE_MS - elapsed);
          }
        });
      } catch {
        // Event listener setup may fail in non-Tauri environments
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
      if (refreshTimerRef.current) {
        clearTimeout(refreshTimerRef.current);
      }
    };
  }, [autoRefresh, repoPath, currentCount, fetchGraphData]);

  // ---------------------------------------------------------------------------
  // Return
  // ---------------------------------------------------------------------------

  return {
    graphLayout,
    commits,
    branches,
    isLoading,
    error,
    loadMore,
    refresh,
    hasMore,
    totalLoaded: commits.length,
  };
}

export default useGitGraph;
