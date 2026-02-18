/**
 * CommitDetail Component
 *
 * Expandable/collapsible panel at the bottom of HistoryTab.
 * Shows when a commit is selected in the graph.
 * Displays full commit info, file changes with +/- stats,
 * and can show diffs via EnhancedDiffViewer.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import type { CommitNode, DiffOutput, FileDiff } from '../../../../types/git';
import { EnhancedDiffViewer } from '../../../ClaudeCodeMode/EnhancedDiffViewer';

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
// CommitDetail Component
// ---------------------------------------------------------------------------

export function CommitDetail({
  commit,
  diff,
  repoPath,
  onClose,
}: CommitDetailProps) {
  const [expandedFile, setExpandedFile] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

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

  return (
    <div className="shrink-0 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 max-h-[40vh] overflow-y-auto">
      {/* Header */}
      <div className="sticky top-0 z-10 flex items-center justify-between px-3 py-2 bg-gray-50 dark:bg-gray-800/80 border-b border-gray-200 dark:border-gray-700 backdrop-blur-sm">
        <div className="flex items-center gap-2">
          <h4 className="text-xs font-medium text-gray-800 dark:text-gray-200">
            Commit Details
          </h4>
          {isMerge && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400 font-medium">
              Merge
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 transition-colors"
          title="Close"
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
          <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">SHA</span>
          <button
            onClick={copySha}
            className={clsx(
              'flex items-center gap-1 text-xs font-mono px-1.5 py-0.5 rounded',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'transition-colors'
            )}
            title="Click to copy full SHA"
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
          <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">Author</span>
          <span className="text-xs text-gray-800 dark:text-gray-200">
            {commit.author_name}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            &lt;{commit.author_email}&gt;
          </span>
        </div>

        {/* Date */}
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">Date</span>
          <span className="text-xs text-gray-700 dark:text-gray-300">
            {formatFullDate(commit.date)}
          </span>
        </div>

        {/* Parents */}
        {commit.parents.length > 0 && (
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-gray-500 dark:text-gray-400 w-12">
              {commit.parents.length > 1 ? 'Parents' : 'Parent'}
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

        {/* AI Summary placeholder (feature-005) */}
        <div className="pt-1">
          <button
            disabled
            className={clsx(
              'flex items-center gap-1.5 px-2 py-1 text-xs rounded-md',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-400 dark:text-gray-500',
              'cursor-not-allowed opacity-60'
            )}
            title="Coming in feature-005"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z"
              />
            </svg>
            AI Summary
          </button>
        </div>
      </div>

      {/* File changes */}
      {diff && diff.files.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          {/* File changes header */}
          <div className="px-3 py-2 bg-gray-50 dark:bg-gray-800/50 flex items-center justify-between">
            <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
              {diff.files.length} file{diff.files.length !== 1 ? 's' : ''} changed
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
          No file changes available for this commit
        </div>
      )}
    </div>
  );
}

export default CommitDetail;
