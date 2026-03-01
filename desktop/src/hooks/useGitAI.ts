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
import { normalizeProvider, DEFAULT_MODEL_BY_PROVIDER } from '../lib/providers';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ReviewNote {
  /** Severity level */
  severity: 'info' | 'warning' | 'error';
  /** Text of the review note */
  text: string;
}

/** Result returned by AI operations — always carries the error string when data is null. */
export interface AIResult<T = string> {
  data: T | null;
  error: string | null;
}

interface UseGitAIReturn {
  /** Whether an LLM provider is configured and available */
  isAvailable: boolean;
  /** Whether the availability check is still loading */
  isCheckingAvailability: boolean;
  /** Tooltip text to show when AI is unavailable */
  unavailableReason: string;

  /** Generate a commit message from staged changes */
  generateCommitMessage: (repoPath: string) => Promise<AIResult>;
  /** Whether commit message generation is in progress */
  isGeneratingCommit: boolean;

  /** Review staged diff */
  reviewDiff: (repoPath: string) => Promise<AIResult>;
  /** Whether review is in progress */
  isReviewing: boolean;

  /** Resolve a conflict file with AI */
  resolveConflictAI: (repoPath: string, filePath: string) => Promise<AIResult>;
  /** Files currently being resolved by AI */
  resolvingFiles: Set<string>;

  /** Summarize a commit */
  summarizeCommit: (repoPath: string, sha: string) => Promise<AIResult>;
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
  const provider = useSettingsStore((s) => s.provider);
  const model = useSettingsStore((s) => s.model);
  const minimaxEndpoint = useSettingsStore((s) => s.minimaxEndpoint);
  const glmEndpoint = useSettingsStore((s) => s.glmEndpoint);
  const qwenEndpoint = useSettingsStore((s) => s.qwenEndpoint);
  const isMountedRef = useRef(true);

  // Configure LLM provider and check availability when settings change
  useEffect(() => {
    isMountedRef.current = true;
    const configureAndCheck = async () => {
      setIsCheckingAvailability(true);
      try {
        // Use provider field if set, otherwise fall back to backend
        const providerName = provider || backend;
        // Only configure if the resolved provider is a direct-API provider
        // (claude-code uses subprocess CLI, not a direct LLM API)
        if (providerName && providerName !== 'claude-code') {
          const canonicalProvider = normalizeProvider(providerName);

          // Resolve model: use store value, or fall back to provider default
          const resolvedModel = model || DEFAULT_MODEL_BY_PROVIDER[canonicalProvider] || '';

          // Resolve provider-specific base_url from zustand endpoint settings
          // (MiniMax China / GLM Coding / Qwen regions have different API endpoints)
          let resolvedBaseUrl: string | undefined;
          if (canonicalProvider === 'minimax' && minimaxEndpoint === 'china') {
            resolvedBaseUrl = 'https://api.minimaxi.com/v1/chat/completions';
          } else if (canonicalProvider === 'glm' && glmEndpoint === 'coding') {
            resolvedBaseUrl = 'https://open.bigmodel.cn/api/coding/paas/v4/chat/completions';
          } else if (canonicalProvider === 'glm' && glmEndpoint === 'international') {
            resolvedBaseUrl = 'https://api.z.ai/api/paas/v4/chat/completions';
          } else if (canonicalProvider === 'glm' && glmEndpoint === 'international-coding') {
            resolvedBaseUrl = 'https://api.z.ai/api/coding/paas/v4/chat/completions';
          } else if (canonicalProvider === 'qwen' && qwenEndpoint === 'singapore') {
            resolvedBaseUrl = 'https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions';
          } else if (canonicalProvider === 'qwen' && qwenEndpoint === 'us') {
            resolvedBaseUrl = 'https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions';
          }

          // Configure the LLM provider on the backend GitState
          const configResult = await invoke<CommandResponse<boolean>>('git_configure_llm', {
            provider: providerName,
            model: resolvedModel,
            // Backend resolves key from secure storage when empty.
            apiKey: '',
            baseUrl: resolvedBaseUrl,
          });

          if (!configResult.success) {
            console.warn('[GitAI] git_configure_llm failed:', configResult.error, {
              provider: providerName,
              model: resolvedModel,
            });
          }
        }
        // Now check availability
        const res = await invoke<CommandResponse<boolean>>('git_check_llm_available', {});
        if (isMountedRef.current) {
          const available = res.success && res.data === true;
          setIsAvailable(available);
          if (!available) {
            console.warn('[GitAI] LLM not available:', { backend, provider, success: res.success, data: res.data });
          }
        }
      } catch (e) {
        console.warn('[GitAI] configureAndCheck exception:', e);
        if (isMountedRef.current) {
          setIsAvailable(false);
        }
      } finally {
        if (isMountedRef.current) {
          setIsCheckingAvailability(false);
        }
      }
    };

    configureAndCheck();
    return () => {
      isMountedRef.current = false;
    };
  }, [backend, provider, model, minimaxEndpoint, glmEndpoint, qwenEndpoint]);

  const unavailableReason = isCheckingAvailability
    ? 'Checking LLM availability...'
    : !isAvailable
      ? 'Configure an LLM provider in Settings to enable AI features'
      : '';

  const clearError = useCallback(() => setLastError(null), []);

  // Generate commit message
  const generateCommitMessage = useCallback(async (repoPath: string): Promise<AIResult> => {
    setIsGeneratingCommit(true);
    setLastError(null);
    try {
      const res = await invoke<CommandResponse<string>>('git_generate_commit_message', {
        repoPath,
      });
      if (res.success && res.data) {
        return { data: res.data, error: null };
      }
      const error = res.error || 'Failed to generate commit message';
      console.warn('[GitAI] generateCommitMessage failed:', error);
      setLastError(error);
      return { data: null, error };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn('[GitAI] generateCommitMessage exception:', msg);
      setLastError(msg);
      return { data: null, error: msg };
    } finally {
      setIsGeneratingCommit(false);
    }
  }, []);

  // Review staged diff
  const reviewDiff = useCallback(async (repoPath: string): Promise<AIResult> => {
    setIsReviewing(true);
    setLastError(null);
    try {
      const res = await invoke<CommandResponse<string>>('git_review_diff', {
        repoPath,
      });
      if (res.success && res.data) {
        return { data: res.data, error: null };
      }
      const error = res.error || 'Failed to review changes';
      console.warn('[GitAI] reviewDiff failed:', error);
      setLastError(error);
      return { data: null, error };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn('[GitAI] reviewDiff exception:', msg);
      setLastError(msg);
      return { data: null, error: msg };
    } finally {
      setIsReviewing(false);
    }
  }, []);

  // Resolve conflict with AI
  const resolveConflictAI = useCallback(async (repoPath: string, filePath: string): Promise<AIResult> => {
    setResolvingFiles((prev) => new Set(prev).add(filePath));
    setLastError(null);
    try {
      const res = await invoke<CommandResponse<string>>('git_resolve_conflict_ai', {
        repoPath,
        filePath,
      });
      if (res.success && res.data) {
        return { data: res.data, error: null };
      }
      const error = res.error || 'Failed to resolve conflict';
      console.warn('[GitAI] resolveConflictAI failed:', error);
      setLastError(error);
      return { data: null, error };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn('[GitAI] resolveConflictAI exception:', msg);
      setLastError(msg);
      return { data: null, error: msg };
    } finally {
      setResolvingFiles((prev) => {
        const next = new Set(prev);
        next.delete(filePath);
        return next;
      });
    }
  }, []);

  // Summarize commit
  const summarizeCommit = useCallback(async (repoPath: string, sha: string): Promise<AIResult> => {
    setIsSummarizing(true);
    setLastError(null);
    try {
      const res = await invoke<CommandResponse<string>>('git_summarize_commit', {
        repoPath,
        sha,
      });
      if (res.success && res.data) {
        return { data: res.data, error: null };
      }
      const error = res.error || 'Failed to summarize commit';
      console.warn('[GitAI] summarizeCommit failed:', error);
      setLastError(error);
      return { data: null, error };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn('[GitAI] summarizeCommit exception:', msg);
      setLastError(msg);
      return { data: null, error: msg };
    } finally {
      setIsSummarizing(false);
    }
  }, []);

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
