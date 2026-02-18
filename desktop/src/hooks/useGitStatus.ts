/**
 * useGitStatus Hook
 *
 * Polls git status on mount, subscribes to Tauri 'git-status-changed' events,
 * and provides debounced auto-refresh after git operations.
 */

import { useEffect, useRef, useCallback } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useGitStore } from '../store/git';
import { useSettingsStore } from '../store/settings';

/** Debounce delay in milliseconds to avoid rapid-fire updates. */
const DEBOUNCE_MS = 500;

/** Polling interval in milliseconds for periodic status refresh. */
const POLL_INTERVAL_MS = 30_000;

export function useGitStatus() {
  const refreshAll = useGitStore((s) => s.refreshAll);
  const status = useGitStore((s) => s.status);
  const isLoading = useGitStore((s) => s.isLoading);
  const error = useGitStore((s) => s.error);
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isMountedRef = useRef(true);

  /**
   * Debounced refresh to prevent rapid sequential updates when file system
   * events fire in quick succession.
   */
  const debouncedRefresh = useCallback(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }
    debounceTimerRef.current = setTimeout(() => {
      if (isMountedRef.current) {
        refreshAll();
      }
    }, DEBOUNCE_MS);
  }, [refreshAll]);

  // Initial fetch + subscribe to Tauri events + polling
  useEffect(() => {
    isMountedRef.current = true;

    if (!workspacePath) return;

    // Initial load
    refreshAll();

    // Subscribe to git-status-changed events from the Rust watcher
    let unlisten: UnlistenFn | null = null;
    const setupListener = async () => {
      try {
        unlisten = await listen('git-status-changed', () => {
          debouncedRefresh();
        });
      } catch {
        // Listener setup may fail if window is closing
      }
    };
    setupListener();

    // Periodic polling as a safety net
    const pollInterval = setInterval(() => {
      if (isMountedRef.current) {
        refreshAll();
      }
    }, POLL_INTERVAL_MS);

    return () => {
      isMountedRef.current = false;
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
      if (unlisten) {
        unlisten();
      }
      clearInterval(pollInterval);
    };
  }, [workspacePath, refreshAll, debouncedRefresh]);

  return {
    status,
    isLoading,
    error,
    refresh: refreshAll,
    debouncedRefresh,
  };
}

export default useGitStatus;
