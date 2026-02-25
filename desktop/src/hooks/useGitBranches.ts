/**
 * useGitBranches Hook
 *
 * Fetches and manages branch list via Tauri IPC.
 * Listens for git events to auto-refresh.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { BranchInfo, RemoteBranchInfo, CommandResponse } from '../types/git';
import { useSettingsStore } from '../store/settings';

interface UseGitBranchesReturn {
  localBranches: BranchInfo[];
  remoteBranches: RemoteBranchInfo[];
  currentBranch: BranchInfo | null;
  isLoading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useGitBranches(): UseGitBranchesReturn {
  const [localBranches, setLocalBranches] = useState<BranchInfo[]>([]);
  const [remoteBranches, setRemoteBranches] = useState<RemoteBranchInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const mountedRef = useRef(true);

  const refresh = useCallback(async () => {
    if (!workspacePath) {
      setLocalBranches([]);
      setRemoteBranches([]);
      setError(null);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      // Fetch local and remote branches in parallel
      const [localRes, remoteRes] = await Promise.all([
        invoke<CommandResponse<BranchInfo[]>>('git_list_branches', {
          repoPath: workspacePath,
        }),
        invoke<CommandResponse<RemoteBranchInfo[]>>('git_list_remote_branches', {
          repoPath: workspacePath,
        }),
      ]);

      if (!mountedRef.current) return;

      if (localRes.success && localRes.data) {
        setLocalBranches(localRes.data);
      } else {
        setError(localRes.error || 'Failed to list local branches');
        setLocalBranches([]);
      }

      if (remoteRes.success && remoteRes.data) {
        setRemoteBranches(remoteRes.data);
      } else {
        // Non-fatal: remote branches may fail if no remotes configured
        setRemoteBranches([]);
      }
    } catch (err) {
      if (!mountedRef.current) return;
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      setLocalBranches([]);
      setRemoteBranches([]);
    } finally {
      if (mountedRef.current) {
        setIsLoading(false);
      }
    }
  }, [workspacePath]);

  // Auto-refresh on mount and workspace change
  useEffect(() => {
    mountedRef.current = true;
    refresh();
    return () => {
      mountedRef.current = false;
    };
  }, [refresh]);

  // Listen for git events to auto-refresh
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setupListeners = async () => {
      try {
        const unsub1 = await listen('git-status-changed', () => {
          if (mountedRef.current) {
            refresh();
          }
        });
        unlisteners.push(unsub1);

        const unsub2 = await listen('git-head-changed', () => {
          if (mountedRef.current) {
            refresh();
          }
        });
        unlisteners.push(unsub2);
      } catch {
        // Events may not be available in test environments
      }
    };

    setupListeners();

    return () => {
      for (const unsub of unlisteners) {
        unsub();
      }
    };
  }, [refresh]);

  const currentBranch = localBranches.find((b) => b.is_head) || null;

  return {
    localBranches,
    remoteBranches,
    currentBranch,
    isLoading,
    error,
    refresh,
  };
}
