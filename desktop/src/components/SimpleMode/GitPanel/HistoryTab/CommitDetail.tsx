/**
 * CommitDetail Component
 *
 * Expandable/collapsible panel at the bottom of HistoryTab.
 * Shows when a commit is selected in the graph.
 * Displays full commit info, file changes with +/- stats,
 * and can show diffs via EnhancedDiffViewer.
 * Includes AI Summary feature (feature-005).
 *
 * Feature-003: Commit History Graph with SVG Visualization
 * Feature-005: AI Commit Summary
 */

import { useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { CommitNode, DiffOutput, FileDiff } from '../../../../types/git';
import { EnhancedDiffViewer } from '../../../ClaudeCodeMode/EnhancedDiffViewer';
import { useGitAI } from '../../../../hooks/useGitAI';
import { useToast } from '../../../shared/Toast';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CommitDetailProps {
  /** The selected commit */
  commit: CommitNode | null;
  /** Diff output for the selected commit */
  diff: DiffOutput | null;
  /** Repository path */
  repoPath: string | null;
  /** Callback to close the detail panel */
  onClose: () => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Format an ISO-8601 date to a human-readable full date.
 */
function formatFullDate(isoDate: string): string {
  const date = new Date(isoDate);
  return date.toLocaleDateString(undefined, {
    weekday: 'short',
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * Count total additions and deletions from a FileDiff's hunks.
 */
function fileDiffStats(file: FileDiff): { additions: number; deletions: number } {
  let additions = 0;
  let deletions = 0;
  for (const hunk of file.hunks) {
    for (const line of hunk.lines) {
      if (line.kind === 'addition') additions++;
      else if (line.kind === 'deletion') deletions++;
    }
  }
  return { additions, deletions };
}

/**
 * Convert a FileDiff to old/new content for EnhancedDiffViewer.
 */
function fileDiffToContents(fileDiff: FileDiff): { oldContent: string; newContent: string } {
  const oldLines: string[] = [];
  const newLines: string[] = [];

  for (const hunk of fileDiff.hunks) {
    for (const line of hunk.lines) {
      if (line.kind === 'context') {
        oldLines.push(line.content);
        newLines.push(line.content);
      } else if (line.kind === 'deletion') {
        oldLines.push(line.content);
      } else if (line.kind === 'addition') {
        newLines.push(line.content);
      }
    }
  }

  return {
    oldContent: oldLines.join('\n'),
    newContent: newLines.join('\n'),
  };
}

// ---------------------------------------------------------------------------
// Spinner
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
// Skeleton
// ---------------------------------------------------------------------------

function SummarySkeleton() {
  return (
    <div className="space-y-2 animate-pulse">
      <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded w-3/4" />
      <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded w-full" />
      <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded w-5/6" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// CommitDetail Component
// ---------------------------------------------------------------------------

export function CommitDetail({
  commit,
  diff,
  repoPath,
  onClose,
}: CommitDetailProps) {
  const { t } = useTranslation('git');
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // AI Summary state
  const { isAvailable, isSummarizing, summarizeCommit, unavailableReason } = useGitAI();
  const { showToast } = useToast();

  // Cache summaries per commit SHA to avoid repeated calls
  const summaryCache = useRef<Map<string, string>>(new Map());
  const [currentSummary, setCurrentSummary] = useState<string | null>(null);
  const [showSummary, setShowSummary] = useState(false);

  // ---------------------------------------------------------------------------
  // Copy SHA to clipboard
  // ---------------------------------------------------------------------------

  const copySha = useCallback(async () => {
    if (!commit) return;
    try {
      await navigator.clipboard.writeText(commit.sha);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API might fail in some contexts
    }
  }, [commit]);

  // ---------------------------------------------------------------------------
  // AI Summary
  // ---------------------------------------------------------------------------

  const handleSummarize = useCallback(async () => {
    if (!commit || !repoPath || !isAvailable) return;

    // Check cache first
    const cached = summaryCache.current.get(commit.sha);
    if (cached) {
      setCurrentSummary(cached);
      setShowSummary(true);
      return;
    }

    const result = await summarizeCommit(repoPath, commit.sha);
    if (result) {
      summaryCache.current.set(commit.sha, result);
      setCurrentSummary(result);
      setShowSummary(true);
      showToast(t('commitDetail.aiSummaryGenerated'), 'success');
    } else {
      showToast(t('commitDetail.aiSummaryFailed'), 'error');
    }
  }, [commit, repoPath, isAvailable, summarizeCommit, showToast]);

  // ---------------------------------------------------------------------------
  // File change status icon
  // ---------------------------------------------------------------------------

  const fileStatusBadge = useCallback((file: FileDiff) => {
    if (file.is_new) {
      return (
        <span className="text-[10px] font-medium px-1 py-0.5 rounded bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400">
          A
        </span>
      );
    }
    if (file.is_deleted) {
      return (
        <span className="text-[10px] font-medium px-1 py-0.5 rounded bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400">
          D
        </span>
      );
    }
    if (file.is_renamed) {
      return (
        <span className="text-[10px] font-medium px-1 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400">
          R
        </span>
      );
    }
    return (
      <span className="text-[10px] font-medium px-1 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400">
        M
      </span>
    );
  }, []);

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  if (!commit) return null;

  const isMerge = commit.parents.length > 1;
  const canSummarize = isAvailable && !isSummarizing;

  return (
    <div className="shrink-0 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 max-h-[40vh] overflow-y-auto">
      {/* Header */}
      <div className="sticky top-0 z-10 flex items-center justify-between px-3 py-2 bg-gray-50 dark:bg-gray-800/80 border-b border-gray-200 dark:border-gray-700 backdrop-blur-sm">
        <div className="flex items-center gap-2">
          <h4 className="text-xs font-medium text-gray-800 dark:text-gray-200">
            {t('commitDetail.title')}
          </h4>
          {isMerge && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400 font-medium">
              {t('commitDetail.merge')}
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 transition-colors"
          title={t('commitDetail.close')}
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Commit info */}
      <div className="px-3 py-2 space-y-2">
        {/* SHA */}
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">{t('commitDetail.sha')}</span>
          <button
            onClick={copySha}
            className={clsx(
              'flex items-center gap-1 text-xs font-mono px-1.5 py-0.5 rounded',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'transition-colors'
            )}
            title={t('commitDetail.copySha')}
          >
            {commit.short_sha}
            {copied ? (
              <svg className="w-3 h-3 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            ) : (
              <svg className="w-3 h-3 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
              </svg>
            )}
          </button>
        </div>

        {/* Author */}
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">{t('commitDetail.author')}</span>
          <span className="text-xs text-gray-800 dark:text-gray-200">
            {commit.author_name}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            &lt;{commit.author_email}&gt;
          </span>
        </div>

        {/* Date */}
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">{t('commitDetail.date')}</span>
          <span className="text-xs text-gray-700 dark:text-gray-300">
            {formatFullDate(commit.date)}
          </span>
        </div>

        {/* Parents */}
        {commit.parents.length > 0 && (
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">
              {commit.parents.length > 1 ? t('commitDetail.parents') : t('commitDetail.parent')}
            </span>
            <div className="flex items-center gap-1.5 flex-wrap">
              {commit.parents.map((parentSha) => (
                <code
                  key={parentSha}
                  className="text-xs font-mono px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400"
                >
                  {parentSha.slice(0, 7)}
                </code>
              ))}
            </div>
          </div>
        )}

        {/* Full message */}
        <div className="pt-1">
          <pre className="text-xs text-gray-800 dark:text-gray-200 whitespace-pre-wrap font-sans leading-relaxed">
            {commit.message}
          </pre>
        </div>

        {/* AI Summary section */}
        <div className="pt-1">
          {!showSummary ? (
            <button
              onClick={handleSummarize}
              disabled={!canSummarize}
              className={clsx(
                'flex items-center gap-1.5 px-2 py-1 text-xs rounded-md transition-colors',
                canSummarize
                  ? 'bg-purple-50 dark:bg-purple-900/20 text-purple-600 dark:text-purple-400 hover:bg-purple-100 dark:hover:bg-purple-900/30 border border-purple-200 dark:border-purple-700'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-500 cursor-not-allowed opacity-60'
              )}
              title={
                !isAvailable
                  ? unavailableReason
                  : isSummarizing
                    ? t('commitDetail.summarizing')
                    : t('commitDetail.generateAiSummary')
              }
            >
              {isSummarizing ? (
                <Spinner className="w-3.5 h-3.5" />
              ) : (
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z"
                  />
                </svg>
              )}
              {isSummarizing ? t('commitDetail.summarizing') : t('commitDetail.aiSummary')}
            </button>
          ) : (
            <div className="rounded-md border border-purple-200 dark:border-purple-800/50 bg-purple-50 dark:bg-purple-900/10 p-2.5">
              <div className="flex items-center justify-between mb-1.5">
                <span className="text-2xs font-medium text-purple-700 dark:text-purple-400 flex items-center gap-1">
                  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z"
                    />
                  </svg>
                  {t('commitDetail.aiSummary')}
                </span>
                <button
                  onClick={() => setShowSummary(false)}
                  className="p-0.5 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                >
                  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
              {isSummarizing ? (
                <SummarySkeleton />
              ) : (
                <pre className="text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap font-sans leading-relaxed">
                  {currentSummary}
                </pre>
              )}
            </div>
          )}
        </div>
      </div>

      {/* File changes */}
      {diff && diff.files.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          {/* File changes header */}
          <div className="px-3 py-2 bg-gray-50 dark:bg-gray-800/50 flex items-center justify-between">
            <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
              {t('commitDetail.filesChanged', { count: diff.files.length })}
            </span>
            <div className="flex items-center gap-2 text-xs">
              <span className="text-green-600 dark:text-green-400">
                +{diff.total_additions}
              </span>
              <span className="text-red-600 dark:text-red-400">
                -{diff.total_deletions}
              </span>
            </div>
          </div>

          {/* File list */}
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {diff.files.map((file) => {
              const stats = fileDiffStats(file);
              const isExpanded = expandedFile === file.path;

              return (
                <div key={file.path}>
                  <button
                    onClick={() =>
                      setExpandedFile(isExpanded ? null : file.path)
                    }
                    className={clsx(
                      'w-full flex items-center gap-2 px-3 py-1.5',
                      'hover:bg-gray-50 dark:hover:bg-gray-800/50',
                      'transition-colors text-left'
                    )}
                  >
                    {/* Expand chevron */}
                    <svg
                      className={clsx(
                        'w-3 h-3 text-gray-400 transition-transform shrink-0',
                        isExpanded && 'rotate-90'
                      )}
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M9 5l7 7-7 7"
                      />
                    </svg>

                    {/* Status badge */}
                    {fileStatusBadge(file)}

                    {/* File path */}
                    <code className="flex-1 text-xs text-gray-700 dark:text-gray-300 truncate">
                      {file.is_renamed && file.old_path
                        ? `${file.old_path} -> ${file.path}`
                        : file.path}
                    </code>

                    {/* +/- stats */}
                    <div className="shrink-0 flex items-center gap-1.5 text-[10px]">
                      {stats.additions > 0 && (
                        <span className="text-green-600 dark:text-green-400">
                          +{stats.additions}
                        </span>
                      )}
                      {stats.deletions > 0 && (
                        <span className="text-red-600 dark:text-red-400">
                          -{stats.deletions}
                        </span>
                      )}
                    </div>
                  </button>

                  {/* Expanded diff */}
                  {isExpanded && (
                    <div className="px-3 pb-2">
                      <EnhancedDiffViewer
                        oldContent={fileDiffToContents(file).oldContent}
                        newContent={fileDiffToContents(file).newContent}
                        filePath={file.path}
                        maxHeight={300}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* No diff data */}
      {(!diff || diff.files.length === 0) && (
        <div className="px-3 py-4 text-center text-xs text-gray-500 dark:text-gray-400 border-t border-gray-200 dark:border-gray-700">
          {t('commitDetail.noFileChanges')}
        </div>
      )}
    </div>
  );
}

export default CommitDetail;
