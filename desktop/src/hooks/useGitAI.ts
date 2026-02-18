/**
 * useGitAI Hook
 *
 * Provides LLM availability detection and AI-powered git operations.
 * Checks if an LLM provider is configured via the backend and
 * conditionally enables/disables all AI buttons.
 *
 * Feature-005: LLM-Powered Git Assistance
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../lib/tauri';
import { useSettingsStore } from '../store/settings';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ReviewNote {
  /** Severity level */
  severity: 'info' | 'warning' | 'error';
  /** Text of the review note */
  text: string;
}

interface UseGitAIReturn {
  /** Whether an LLM provider is configured and available */
  isAvailable: boolean;
  /** Whether the availability check is still loading */
  isCheckingAvailability: boolean;
  /** Tooltip text to show when AI is unavailable */
  unavailableReason: string;

  /** Generate a commit message from staged changes */
  generateCommitMessage: (repoPath: string) => Promise<string | null>;
  /** Whether commit message generation is in progress */
  isGeneratingCommit: boolean;

  /** Review staged diff */
  reviewDiff: (repoPath: string) => Promise<string | null>;
  /** Whether review is in progress */
  isReviewing: boolean;

  /** Resolve a conflict file with AI */
  resolveConflictAI: (repoPath: string, filePath: string) => Promise<string | null>;
  /** Files currently being resolved by AI */
  resolvingFiles: Set<string>;

  /** Summarize a commit */
  summarizeCommit: (repoPath: string, sha: string) => Promise<string | null>;
  /** Whether summarization is in progress */
  isSummarizing: boolean;

  /** Last error message from any AI operation */
  lastError: string | null;
  /** Clear the last error */
  clearError: () => void;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useGitAI(): UseGitAIReturn {
  const [isAvailable, setIsAvailable] = useState(false);
  const [isCheckingAvailability, setIsCheckingAvailability] = useState(true);
  const [isGeneratingCommit, setIsGeneratingCommit] = useState(false);
  const [isReviewing, setIsReviewing] = useState(false);
  const [resolvingFiles, setResolvingFiles] = useState<Set<string>>(new Set());
  const [isSummarizing, setIsSummarizing] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const backend = useSettingsStore((s) => s.backend);
  const apiKey = useSettingsStore((s) => s.apiKey);
  const isMountedRef = useRef(true);

  // Check LLM availability when backend/apiKey changes
  useEffect(() => {
    isMountedRef.current = true;
    const checkAvailability = async () => {
      setIsCheckingAvailability(true);
      try {
        const res = await invoke<CommandResponse<boolean>>('git_check_llm_available', {});
        if (isMountedRef.current) {
          setIsAvailable(res.success && res.data === true);
        }
      } catch {
        if (isMountedRef.current) {
          setIsAvailable(false);
        }
      } finally {
        if (isMountedRef.current) {
          setIsCheckingAvailability(false);
        }
      }
    };

    checkAvailability();
    return () => {
      isMountedRef.current = false;
    };
  }, [backend, apiKey]);

  const unavailableReason = isCheckingAvailability
    ? 'Checking LLM availability...'
    : !isAvailable
      ? 'Configure an LLM provider in Settings to enable AI features'
      : '';

  const clearError = useCallback(() => setLastError(null), []);

  // Generate commit message
  const generateCommitMessage = useCallback(async (repoPath: string): Promise<string | null> => {
    setIsGeneratingCommit(true);
    setLastError(null);
    try {
      const res = await invoke<CommandResponse<string>>('git_generate_commit_message', {
        repoPath,
      });
      if (res.success && res.data) {
        return res.data;
      }
      setLastError(res.error || 'Failed to generate commit message');
      return null;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setLastError(msg);
      return null;
    } finally {
      setIsGeneratingCommit(false);
    }
  }, []);

  // Review staged diff
  const reviewDiff = useCallback(async (repoPath: string): Promise<string | null> => {
    setIsReviewing(true);
    setLastError(null);
    try {
      const res = await invoke<CommandResponse<string>>('git_review_diff', {
        repoPath,
      });
      if (res.success && res.data) {
        return res.data;
      }
      setLastError(res.error || 'Failed to review changes');
      return null;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setLastError(msg);
      return null;
    } finally {
      setIsReviewing(false);
    }
  }, []);

  // Resolve conflict with AI
  const resolveConflictAI = useCallback(
    async (repoPath: string, filePath: string): Promise<string | null> => {
      setResolvingFiles((prev) => new Set(prev).add(filePath));
      setLastError(null);
      try {
        const res = await invoke<CommandResponse<string>>('git_resolve_conflict_ai', {
          repoPath,
          filePath,
        });
        if (res.success && res.data) {
          return res.data;
        }
        setLastError(res.error || 'Failed to resolve conflict');
        return null;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setLastError(msg);
        return null;
      } finally {
        setResolvingFiles((prev) => {
          const next = new Set(prev);
          next.delete(filePath);
          return next;
        });
      }
    },
    []
  );

  // Summarize commit
  const summarizeCommit = useCallback(
    async (repoPath: string, sha: string): Promise<string | null> => {
      setIsSummarizing(true);
      setLastError(null);
      try {
        const res = await invoke<CommandResponse<string>>('git_summarize_commit', {
          repoPath,
          sha,
        });
        if (res.success && res.data) {
          return res.data;
        }
        setLastError(res.error || 'Failed to summarize commit');
        return null;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setLastError(msg);
        return null;
      } finally {
        setIsSummarizing(false);
      }
    },
    []
  );

  return {
    isAvailable,
    isCheckingAvailability,
    unavailableReason,
    generateCommitMessage,
    isGeneratingCommit,
    reviewDiff,
    isReviewing,
    resolveConflictAI,
    resolvingFiles,
    summarizeCommit,
    isSummarizing,
    lastError,
    clearError,
  };
}

export default useGitAI;
