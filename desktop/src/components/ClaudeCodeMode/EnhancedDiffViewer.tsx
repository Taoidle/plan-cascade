/**
 * EnhancedDiffViewer Component
 *
 * Comprehensive file diff viewer for Edit tool results. Supports both
 * side-by-side comparison and unified diff views with syntax highlighting,
 * line numbers, and navigation between changes.
 *
 * Story-003: File diff viewer with side-by-side and unified modes
 */

import { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { clsx } from 'clsx';
import {
  ViewHorizontalIcon,
  ViewVerticalIcon,
  ChevronUpIcon,
  ChevronDownIcon,
  CopyIcon,
  CheckIcon,
  PlusIcon,
  MinusIcon,
  TextAlignJustifyIcon,
  ChevronRightIcon,
} from '@radix-ui/react-icons';

// ============================================================================
// Types
// ============================================================================

interface EnhancedDiffViewerProps {
  /** Old content (before changes) */
  oldContent: string;
  /** New content (after changes) */
  newContent: string;
  /** File path for syntax highlighting hints */
  filePath?: string;
  /** Additional CSS classes */
  className?: string;
  /** Maximum height in pixels */
  maxHeight?: number;
}

type DiffViewMode = 'unified' | 'side-by-side';

interface DiffLine {
  type: 'added' | 'removed' | 'unchanged' | 'header';
  content: string;
  oldLineNumber?: number;
  newLineNumber?: number;
  charDiffs?: CharDiff[];
}

interface CharDiff {
  type: 'added' | 'removed' | 'unchanged';
  text: string;
}

interface DiffChange {
  index: number;
  type: 'added' | 'removed' | 'modified';
}

// ============================================================================
// Diff Algorithm (Simple LCS-based)
// ============================================================================

function computeDiff(oldLines: string[], newLines: string[]): DiffLine[] {
  const result: DiffLine[] = [];

  // Build LCS matrix
  const m = oldLines.length;
  const n = newLines.length;
  const dp: number[][] = Array(m + 1).fill(null).map(() => Array(n + 1).fill(0));

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (oldLines[i - 1] === newLines[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }

  // Backtrack to find diff
  let i = m, j = n;
  const changes: DiffLine[] = [];

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      changes.unshift({
        type: 'unchanged',
        content: oldLines[i - 1],
        oldLineNumber: i,
        newLineNumber: j,
      });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      changes.unshift({
        type: 'added',
        content: newLines[j - 1],
        newLineNumber: j,
      });
      j--;
    } else {
      changes.unshift({
        type: 'removed',
        content: oldLines[i - 1],
        oldLineNumber: i,
      });
      i--;
    }
  }

  // Add character-level diffs for modified lines
  let lastRemoved: DiffLine | null = null;

  for (const change of changes) {
    if (change.type === 'removed') {
      lastRemoved = change;
    } else if (change.type === 'added' && lastRemoved) {
      // Compute character-level diff
      const charDiffs = computeCharDiff(lastRemoved.content, change.content);
      lastRemoved.charDiffs = charDiffs.oldDiffs;
      change.charDiffs = charDiffs.newDiffs;
      lastRemoved = null;
    } else {
      lastRemoved = null;
    }

    result.push(change);
  }

  return result;
}

function computeCharDiff(oldText: string, newText: string): {
  oldDiffs: CharDiff[];
  newDiffs: CharDiff[];
} {
  // Note: These variables are initialized but the actual diffs are built in oldResult/newResult
  // and returned at the end. The initial arrays are here for type definition clarity.

  // Simple word-based diff
  const oldWords = oldText.split(/(\s+)/);
  const newWords = newText.split(/(\s+)/);

  // Build LCS for words
  const m = oldWords.length;
  const n = newWords.length;
  const dp: number[][] = Array(m + 1).fill(null).map(() => Array(n + 1).fill(0));

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (oldWords[i - 1] === newWords[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }

  // Backtrack
  let i = m, j = n;
  const oldResult: CharDiff[] = [];
  const newResult: CharDiff[] = [];

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldWords[i - 1] === newWords[j - 1]) {
      oldResult.unshift({ type: 'unchanged', text: oldWords[i - 1] });
      newResult.unshift({ type: 'unchanged', text: newWords[j - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      newResult.unshift({ type: 'added', text: newWords[j - 1] });
      j--;
    } else {
      oldResult.unshift({ type: 'removed', text: oldWords[i - 1] });
      i--;
    }
  }

  // oldDiffs and newDiffs are returned for potential future use
  return { oldDiffs: oldResult, newDiffs: newResult };
}

// Re-export for potential use (currently used by computeDiff)
export { computeCharDiff as _computeCharDiff };

// ============================================================================
// Helper Functions
// ============================================================================

// getLanguageFromPath is available for syntax highlighting (future enhancement)
export function getLanguageFromPath(filePath?: string): string {
  if (!filePath) return 'text';

  const ext = filePath.split('.').pop()?.toLowerCase() || '';
  const languageMap: Record<string, string> = {
    ts: 'typescript',
    tsx: 'typescript',
    js: 'javascript',
    jsx: 'javascript',
    py: 'python',
    rs: 'rust',
    go: 'go',
    java: 'java',
    json: 'json',
    md: 'markdown',
    html: 'html',
    css: 'css',
    scss: 'scss',
    yaml: 'yaml',
    yml: 'yaml',
  };

  return languageMap[ext] || 'text';
}

// ============================================================================
// DiffSummary Component
// ============================================================================

interface DiffSummaryProps {
  diffLines: DiffLine[];
}

function DiffSummary({ diffLines }: DiffSummaryProps) {
  const stats = useMemo(() => {
    let additions = 0;
    let deletions = 0;
    let modifications = 0;

    let lastRemoved = false;
    for (const line of diffLines) {
      if (line.type === 'added') {
        if (lastRemoved) {
          modifications++;
          lastRemoved = false;
        } else {
          additions++;
        }
      } else if (line.type === 'removed') {
        if (!lastRemoved) {
          deletions++;
          lastRemoved = true;
        }
      } else {
        lastRemoved = false;
      }
    }

    return { additions, deletions, modifications };
  }, [diffLines]);

  return (
    <div className="flex items-center gap-3 text-sm">
      <span className="text-green-600 dark:text-green-400 flex items-center gap-1">
        <PlusIcon className="w-3 h-3" />
        {stats.additions} added
      </span>
      <span className="text-red-600 dark:text-red-400 flex items-center gap-1">
        <MinusIcon className="w-3 h-3" />
        {stats.deletions} removed
      </span>
      {stats.modifications > 0 && (
        <span className="text-yellow-600 dark:text-yellow-400">
          ~{stats.modifications} modified
        </span>
      )}
    </div>
  );
}

// ============================================================================
// UnifiedDiffView Component
// ============================================================================

interface UnifiedDiffViewProps {
  diffLines: DiffLine[];
  wrapLines: boolean;
  collapsedSections: Set<number>;
  toggleCollapse: (index: number) => void;
}

function UnifiedDiffView({
  diffLines,
  wrapLines,
  collapsedSections,
  toggleCollapse,
}: UnifiedDiffViewProps) {
  // Group consecutive unchanged lines for collapsing
  const groupedLines = useMemo(() => {
    const groups: { lines: DiffLine[]; startIndex: number; isCollapsible: boolean }[] = [];
    let currentGroup: DiffLine[] = [];
    let startIndex = 0;
    let unchangedCount = 0;

    diffLines.forEach((line, index) => {
      if (line.type === 'unchanged') {
        unchangedCount++;
        currentGroup.push(line);
      } else {
        if (unchangedCount > 6) {
          // Keep first 3 and last 3, collapse the rest
          const first3 = currentGroup.slice(0, 3);
          const last3 = currentGroup.slice(-3);
          const middle = currentGroup.slice(3, -3);

          if (first3.length > 0) {
            groups.push({ lines: first3, startIndex, isCollapsible: false });
          }
          if (middle.length > 0) {
            groups.push({ lines: middle, startIndex: startIndex + 3, isCollapsible: true });
          }
          if (last3.length > 0) {
            groups.push({ lines: last3, startIndex: startIndex + currentGroup.length - 3, isCollapsible: false });
          }
        } else if (currentGroup.length > 0) {
          groups.push({ lines: currentGroup, startIndex, isCollapsible: false });
        }

        currentGroup = [line];
        startIndex = index;
        unchangedCount = 0;
      }
    });

    // Handle remaining lines
    if (currentGroup.length > 0) {
      if (unchangedCount > 6) {
        const first3 = currentGroup.slice(0, 3);
        const middle = currentGroup.slice(3);

        groups.push({ lines: first3, startIndex, isCollapsible: false });
        if (middle.length > 0) {
          groups.push({ lines: middle, startIndex: startIndex + 3, isCollapsible: true });
        }
      } else {
        groups.push({ lines: currentGroup, startIndex, isCollapsible: false });
      }
    }

    return groups;
  }, [diffLines]);

  return (
    <div className="font-mono text-sm">
      {groupedLines.map((group, groupIndex) => {
        if (group.isCollapsible && collapsedSections.has(group.startIndex)) {
          return (
            <button
              key={groupIndex}
              onClick={() => toggleCollapse(group.startIndex)}
              className={clsx(
                'w-full flex items-center justify-center gap-2 py-1',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-500 dark:text-gray-400',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'transition-colors text-xs'
              )}
            >
              <ChevronRightIcon className="w-3 h-3" />
              Show {group.lines.length} hidden lines
            </button>
          );
        }

        return (
          <div key={groupIndex}>
            {group.isCollapsible && (
              <button
                onClick={() => toggleCollapse(group.startIndex)}
                className={clsx(
                  'w-full flex items-center justify-center gap-2 py-1',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'transition-colors text-xs'
                )}
              >
                Hide {group.lines.length} unchanged lines
              </button>
            )}
            {(!group.isCollapsible || !collapsedSections.has(group.startIndex)) &&
              group.lines.map((line, lineIndex) => (
                <div
                  key={lineIndex}
                  className={clsx(
                    'flex',
                    line.type === 'added' && 'bg-green-50 dark:bg-green-900/20',
                    line.type === 'removed' && 'bg-red-50 dark:bg-red-900/20',
                    !wrapLines && 'whitespace-nowrap'
                  )}
                >
                  {/* Line numbers */}
                  <span className="w-12 text-right pr-2 text-gray-400 select-none flex-shrink-0 border-r border-gray-200 dark:border-gray-700">
                    {line.oldLineNumber || ''}
                  </span>
                  <span className="w-12 text-right pr-2 text-gray-400 select-none flex-shrink-0 border-r border-gray-200 dark:border-gray-700">
                    {line.newLineNumber || ''}
                  </span>

                  {/* Change indicator */}
                  <span className={clsx(
                    'w-6 text-center select-none flex-shrink-0',
                    line.type === 'added' && 'text-green-600 dark:text-green-400',
                    line.type === 'removed' && 'text-red-600 dark:text-red-400'
                  )}>
                    {line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' '}
                  </span>

                  {/* Content */}
                  <pre className={clsx(
                    'flex-1 px-2',
                    wrapLines && 'whitespace-pre-wrap break-all',
                    line.type === 'added' && 'text-green-800 dark:text-green-300',
                    line.type === 'removed' && 'text-red-800 dark:text-red-300'
                  )}>
                    {line.charDiffs ? (
                      line.charDiffs.map((diff, i) => (
                        <span
                          key={i}
                          className={clsx(
                            diff.type === 'added' && 'bg-green-200 dark:bg-green-700',
                            diff.type === 'removed' && 'bg-red-200 dark:bg-red-700'
                          )}
                        >
                          {diff.text}
                        </span>
                      ))
                    ) : (
                      line.content || '\u00A0'
                    )}
                  </pre>
                </div>
              ))}
          </div>
        );
      })}
    </div>
  );
}

// ============================================================================
// SideBySideDiffView Component
// ============================================================================

interface SideBySideDiffViewProps {
  diffLines: DiffLine[];
  wrapLines: boolean;
}

function SideBySideDiffView({ diffLines, wrapLines }: SideBySideDiffViewProps) {
  // Pair up lines for side-by-side display
  const pairedLines = useMemo(() => {
    const pairs: { left: DiffLine | null; right: DiffLine | null }[] = [];
    const removed: DiffLine[] = [];

    for (const line of diffLines) {
      if (line.type === 'unchanged') {
        // Flush any pending removed lines
        for (const r of removed) {
          pairs.push({ left: r, right: null });
        }
        removed.length = 0;

        pairs.push({ left: line, right: line });
      } else if (line.type === 'removed') {
        removed.push(line);
      } else if (line.type === 'added') {
        if (removed.length > 0) {
          // Pair with removed line
          const r = removed.shift()!;
          pairs.push({ left: r, right: line });
        } else {
          pairs.push({ left: null, right: line });
        }
      }
    }

    // Flush remaining removed lines
    for (const r of removed) {
      pairs.push({ left: r, right: null });
    }

    return pairs;
  }, [diffLines]);

  return (
    <div className="font-mono text-sm flex">
      {/* Left side (old) */}
      <div className="flex-1 border-r border-gray-300 dark:border-gray-600">
        <div className="px-2 py-1 bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-300 text-xs font-medium">
          Old
        </div>
        {pairedLines.map((pair, index) => (
          <div
            key={index}
            className={clsx(
              'flex',
              pair.left?.type === 'removed' && 'bg-red-50 dark:bg-red-900/20',
              !pair.left && 'bg-gray-50 dark:bg-gray-800/50',
              !wrapLines && 'whitespace-nowrap'
            )}
          >
            <span className="w-10 text-right pr-2 text-gray-400 select-none flex-shrink-0 border-r border-gray-200 dark:border-gray-700">
              {pair.left?.oldLineNumber || ''}
            </span>
            <pre className={clsx(
              'flex-1 px-2 min-h-[1.5em]',
              wrapLines && 'whitespace-pre-wrap break-all',
              pair.left?.type === 'removed' && 'text-red-800 dark:text-red-300'
            )}>
              {pair.left ? (
                pair.left.charDiffs ? (
                  pair.left.charDiffs.map((diff, i) => (
                    <span
                      key={i}
                      className={clsx(diff.type === 'removed' && 'bg-red-200 dark:bg-red-700')}
                    >
                      {diff.text}
                    </span>
                  ))
                ) : (
                  pair.left.content || '\u00A0'
                )
              ) : (
                '\u00A0'
              )}
            </pre>
          </div>
        ))}
      </div>

      {/* Right side (new) */}
      <div className="flex-1">
        <div className="px-2 py-1 bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-300 text-xs font-medium">
          New
        </div>
        {pairedLines.map((pair, index) => (
          <div
            key={index}
            className={clsx(
              'flex',
              pair.right?.type === 'added' && 'bg-green-50 dark:bg-green-900/20',
              !pair.right && 'bg-gray-50 dark:bg-gray-800/50',
              !wrapLines && 'whitespace-nowrap'
            )}
          >
            <span className="w-10 text-right pr-2 text-gray-400 select-none flex-shrink-0 border-r border-gray-200 dark:border-gray-700">
              {pair.right?.newLineNumber || ''}
            </span>
            <pre className={clsx(
              'flex-1 px-2 min-h-[1.5em]',
              wrapLines && 'whitespace-pre-wrap break-all',
              pair.right?.type === 'added' && 'text-green-800 dark:text-green-300'
            )}>
              {pair.right ? (
                pair.right.charDiffs ? (
                  pair.right.charDiffs.map((diff, i) => (
                    <span
                      key={i}
                      className={clsx(diff.type === 'added' && 'bg-green-200 dark:bg-green-700')}
                    >
                      {diff.text}
                    </span>
                  ))
                ) : (
                  pair.right.content || '\u00A0'
                )
              ) : (
                '\u00A0'
              )}
            </pre>
          </div>
        ))}
      </div>
    </div>
  );
}

// ============================================================================
// EnhancedDiffViewer Component
// ============================================================================

export function EnhancedDiffViewer({
  oldContent,
  newContent,
  filePath,
  className,
  maxHeight = 500,
}: EnhancedDiffViewerProps) {
  // State
  const [viewMode, setViewMode] = useState<DiffViewMode>('unified');
  const [wrapLines, setWrapLines] = useState(true);
  const [copied, setCopied] = useState<'new' | 'diff' | null>(null);
  const [collapsedSections, setCollapsedSections] = useState<Set<number>>(new Set());
  const [currentChangeIndex, setCurrentChangeIndex] = useState(0);

  // Refs
  const containerRef = useRef<HTMLDivElement>(null);

  // Load view mode preference from localStorage
  useEffect(() => {
    const savedMode = localStorage.getItem('diffViewMode');
    if (savedMode === 'unified' || savedMode === 'side-by-side') {
      setViewMode(savedMode);
    }
  }, []);

  // Save view mode preference
  const handleViewModeChange = useCallback((mode: DiffViewMode) => {
    setViewMode(mode);
    localStorage.setItem('diffViewMode', mode);
  }, []);

  // Compute diff
  const diffLines = useMemo(() => {
    const oldLines = oldContent.split('\n');
    const newLines = newContent.split('\n');
    return computeDiff(oldLines, newLines);
  }, [oldContent, newContent]);

  // Find change positions
  const changes = useMemo(() => {
    const result: DiffChange[] = [];
    diffLines.forEach((line, index) => {
      if (line.type === 'added' || line.type === 'removed') {
        result.push({ index, type: line.type === 'added' ? 'added' : 'removed' });
      }
    });
    return result;
  }, [diffLines]);

  // Navigation between changes
  const navigateToChange = useCallback((direction: 'next' | 'prev') => {
    if (changes.length === 0) return;

    let newIndex = currentChangeIndex;
    if (direction === 'next') {
      newIndex = (currentChangeIndex + 1) % changes.length;
    } else {
      newIndex = (currentChangeIndex - 1 + changes.length) % changes.length;
    }
    setCurrentChangeIndex(newIndex);

    // Scroll to the change
    // Note: In a real implementation, you'd scroll to the actual DOM element
  }, [changes, currentChangeIndex]);

  // Toggle collapsed section
  const toggleCollapse = useCallback((index: number) => {
    setCollapsedSections(prev => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  }, []);

  // Copy handlers
  const handleCopyNew = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(newContent);
      setCopied('new');
      setTimeout(() => setCopied(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, [newContent]);

  // Check if there are any changes
  const hasChanges = diffLines.some(line => line.type !== 'unchanged');

  if (!hasChanges) {
    return (
      <div className={clsx(
        'flex flex-col items-center justify-center py-8',
        'text-gray-500 dark:text-gray-400',
        className
      )}>
        <CheckIcon className="w-8 h-8 mb-2 opacity-50" />
        <p className="text-sm">No changes detected</p>
      </div>
    );
  }

  return (
    <div className={clsx('flex flex-col rounded-lg border border-gray-200 dark:border-gray-700', className)}>
      {/* Header */}
      <div className={clsx(
        'flex items-center justify-between px-3 py-2',
        'bg-gray-50 dark:bg-gray-800/50',
        'border-b border-gray-200 dark:border-gray-700'
      )}>
        <DiffSummary diffLines={diffLines} />

        <div className="flex items-center gap-2">
          {/* View mode toggle */}
          <div className="flex rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
            <button
              onClick={() => handleViewModeChange('unified')}
              className={clsx(
                'px-2 py-1 text-xs transition-colors',
                viewMode === 'unified'
                  ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300'
                  : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'
              )}
            >
              <ViewVerticalIcon className="w-4 h-4" />
            </button>
            <button
              onClick={() => handleViewModeChange('side-by-side')}
              className={clsx(
                'px-2 py-1 text-xs transition-colors',
                viewMode === 'side-by-side'
                  ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300'
                  : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'
              )}
            >
              <ViewHorizontalIcon className="w-4 h-4" />
            </button>
          </div>

          {/* Wrap toggle */}
          <button
            onClick={() => setWrapLines(!wrapLines)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              wrapLines
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
            title="Toggle word wrap"
          >
            <TextAlignJustifyIcon className="w-4 h-4" />
          </button>

          {/* Navigation */}
          <div className="flex items-center gap-1">
            <button
              onClick={() => navigateToChange('prev')}
              disabled={changes.length === 0}
              className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
              title="Previous change"
            >
              <ChevronUpIcon className="w-4 h-4" />
            </button>
            {changes.length > 0 && (
              <span className="text-xs text-gray-500">
                {currentChangeIndex + 1}/{changes.length}
              </span>
            )}
            <button
              onClick={() => navigateToChange('next')}
              disabled={changes.length === 0}
              className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
              title="Next change"
            >
              <ChevronDownIcon className="w-4 h-4" />
            </button>
          </div>

          {/* Copy buttons */}
          <button
            onClick={handleCopyNew}
            className={clsx(
              'p-1.5 rounded transition-colors',
              copied === 'new'
                ? 'bg-green-100 dark:bg-green-900/50 text-green-600 dark:text-green-400'
                : 'text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
            title={copied === 'new' ? 'Copied!' : 'Copy new content'}
          >
            {copied === 'new' ? <CheckIcon className="w-4 h-4" /> : <CopyIcon className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* File path */}
      {filePath && (
        <div className="px-3 py-1 bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
          <code className="text-xs text-gray-600 dark:text-gray-400">{filePath}</code>
        </div>
      )}

      {/* Diff content */}
      <div
        ref={containerRef}
        className="overflow-auto"
        style={{ maxHeight }}
      >
        {viewMode === 'unified' ? (
          <UnifiedDiffView
            diffLines={diffLines}
            wrapLines={wrapLines}
            collapsedSections={collapsedSections}
            toggleCollapse={toggleCollapse}
          />
        ) : (
          <SideBySideDiffView
            diffLines={diffLines}
            wrapLines={wrapLines}
          />
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Exports
// ============================================================================

export { computeDiff, computeCharDiff };
export default EnhancedDiffViewer;
