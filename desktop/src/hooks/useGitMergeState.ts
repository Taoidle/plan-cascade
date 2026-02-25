/**
 * useGitMergeState Hook
 *
 * Monitors merge state and triggers refresh across tabs.
 * Listens for git events to detect merge state changes.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useGitStore } from '../store/git';
import { useSettingsStore } from '../store/settings';

/**
 * Initializes merge state monitoring.
 * Should be called once at the top level where git panel is used.
 */
export function useGitMergeState() {
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const refreshMergeState = useGitStore((s) => s.refreshMergeState);
  const mountedRef = useRef(true);

  // Check merge state on mount and workspace change
  useEffect(() => {
    mountedRef.current = true;
    if (workspacePath) {
      refreshMergeState();
    }
    return () => {
      mountedRef.current = false;
    };
  }, [workspacePath, refreshMergeState]);

  // Listen for git events
  useEffect(() => {
    if (!workspacePath) return;

    const unlisteners: UnlistenFn[] = [];

    const setup = async () => {
      try {
        const unsub1 = await listen('git-status-changed', () => {
          if (mountedRef.current && workspacePath) {
            refreshMergeState();
          }
        });
        unlisteners.push(unsub1);

        const unsub2 = await listen('git-head-changed', () => {
          if (mountedRef.current && workspacePath) {
            refreshMergeState();
          }
        });
        unlisteners.push(unsub2);
      } catch {
        // Events may not be available in test environments
      }
    };

    setup();

    return () => {
      for (const unsub of unlisteners) {
        unsub();
      }
    };
  }, [workspacePath, refreshMergeState]);

  return useGitStore((s) => ({
    isInMerge: s.isInMerge,
    mergeState: s.mergeState,
    conflictFiles: s.conflictFiles,
  }));
}
