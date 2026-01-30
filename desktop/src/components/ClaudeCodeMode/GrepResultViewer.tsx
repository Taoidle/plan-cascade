/**
 * GrepResultViewer Component
 *
 * Specialized result viewer for Grep tool results. Shows matched content
 * with file paths, line numbers, pattern highlighting, and context lines.
 *
 * Story-005: Glob and Grep result viewer with file list
 */

import { useState, useMemo, useCallback } from 'react';
import { clsx } from 'clsx';
import {
  ChevronRightIcon,
  ChevronDownIcon,
  MagnifyingGlassIcon,
  CopyIcon,
  CheckIcon,
  Cross2Icon,
  ExternalLinkIcon,
} from '@radix-ui/react-icons';
import { getFileIcon } from './GlobResultViewer';

// ============================================================================
// Types
// ============================================================================

interface GrepMatch {
  file: string;
  line: number;
  content: string;
  contextBefore?: string[];
  contextAfter?: string[];
}

interface GrepResultViewerProps {
  /** List of grep matches */
  matches: GrepMatch[];
  /** The search pattern used */
  pattern?: string;
  /** Output mode: content shows matches, files_with_matches shows only files */
  outputMode?: 'content' | 'files_with_matches';
  /** Files list (when outputMode is files_with_matches) */
  files?: string[];
  /** Callback when a file/line is clicked (for preview integration) */
  onMatchClick?: (filePath: string, lineNumber?: number) => void;
  /** Maximum height in pixels */
  maxHeight?: number;
  /** Additional CSS classes */
  className?: string;
}

interface GroupedMatches {
  file: string;
  matches: GrepMatch[];
}

// ============================================================================
// Highlight Helper
// ============================================================================

function highlightPattern(text: string, pattern: string): JSX.Element[] {
  if (!pattern) {
    return [<span key="0">{text}</span>];
  }

  try {
    // Try to use the pattern as regex
    const regex = new RegExp(`(${pattern})`, 'gi');
    const parts = text.split(regex);

    return parts.map((part, index) => {
      const isMatch = regex.test(part);
      // Reset regex state
      regex.lastIndex = 0;

      if (isMatch) {
        return (
          <mark
            key={index}
            className="bg-yellow-200 dark:bg-yellow-800 text-yellow-900 dark:text-yellow-100 rounded px-0.5"
          >
            {part}
          </mark>
        );
      }
      return <span key={index}>{part}</span>;
    });
  } catch {
    // If pattern is not a valid regex, do simple string matching
    const lowerText = text.toLowerCase();
    const lowerPattern = pattern.toLowerCase();
    const parts: JSX.Element[] = [];
    let lastIndex = 0;

    let index = lowerText.indexOf(lowerPattern, lastIndex);
    while (index !== -1) {
      // Add text before match
      if (index > lastIndex) {
        parts.push(<span key={`text-${lastIndex}`}>{text.slice(lastIndex, index)}</span>);
      }

      // Add highlighted match
      parts.push(
        <mark
          key={`match-${index}`}
          className="bg-yellow-200 dark:bg-yellow-800 text-yellow-900 dark:text-yellow-100 rounded px-0.5"
        >
          {text.slice(index, index + pattern.length)}
        </mark>
      );

      lastIndex = index + pattern.length;
      index = lowerText.indexOf(lowerPattern, lastIndex);
    }

    // Add remaining text
    if (lastIndex < text.length) {
      parts.push(<span key={`text-${lastIndex}`}>{text.slice(lastIndex)}</span>);
    }

    return parts.length > 0 ? parts : [<span key="0">{text}</span>];
  }
}

// ============================================================================
// FileMatchGroup Component
// ============================================================================

interface FileMatchGroupProps {
  group: GroupedMatches;
  pattern?: string;
  onMatchClick?: (filePath: string, lineNumber?: number) => void;
  defaultExpanded?: boolean;
  showContext?: boolean;
}

function FileMatchGroup({
  group,
  pattern,
  onMatchClick,
  defaultExpanded = true,
  showContext = true,
}: FileMatchGroupProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [showFullContext, setShowFullContext] = useState(false);

  const filename = group.file.split(/[/\\]/).pop() || group.file;
  const { Icon, color } = getFileIcon(filename);

  return (
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      {/* File header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={clsx(
          'w-full flex items-center gap-2 px-3 py-2',
          'bg-gray-50 dark:bg-gray-800',
          'hover:bg-gray-100 dark:hover:bg-gray-700',
          'transition-colors text-left'
        )}
      >
        {isExpanded ? (
          <ChevronDownIcon className="w-4 h-4 text-gray-400" />
        ) : (
          <ChevronRightIcon className="w-4 h-4 text-gray-400" />
        )}
        <Icon className={clsx('w-4 h-4', color)} />
        <span className="flex-1 text-sm font-medium text-gray-700 dark:text-gray-300 truncate">
          {group.file}
        </span>
        <span className="text-xs text-gray-500 px-2 py-0.5 bg-gray-200 dark:bg-gray-700 rounded-full">
          {group.matches.length} match{group.matches.length !== 1 ? 'es' : ''}
        </span>
      </button>

      {/* Matches */}
      {isExpanded && (
        <div className="divide-y divide-gray-100 dark:divide-gray-800">
          {group.matches.map((match, index) => (
            <div
              key={index}
              className="bg-white dark:bg-gray-900"
            >
              {/* Context before */}
              {showContext && showFullContext && match.contextBefore?.map((ctx, ctxIndex) => (
                <div
                  key={`before-${ctxIndex}`}
                  className="flex items-start px-3 py-1 text-gray-400 dark:text-gray-500"
                >
                  <span className="w-12 flex-shrink-0 text-right pr-3 text-xs font-mono">
                    {match.line - (match.contextBefore!.length - ctxIndex)}
                  </span>
                  <pre className="flex-1 text-xs font-mono overflow-x-auto whitespace-pre">
                    {ctx}
                  </pre>
                </div>
              ))}

              {/* Match line */}
              <button
                onClick={() => onMatchClick?.(group.file, match.line)}
                className={clsx(
                  'w-full flex items-start px-3 py-1.5 text-left',
                  'hover:bg-yellow-50 dark:hover:bg-yellow-900/20',
                  'transition-colors'
                )}
              >
                <span className="w-12 flex-shrink-0 text-right pr-3 text-xs font-mono text-primary-600 dark:text-primary-400 font-medium">
                  {match.line}
                </span>
                <pre className="flex-1 text-sm font-mono overflow-x-auto whitespace-pre">
                  {highlightPattern(match.content, pattern || '')}
                </pre>
                <ExternalLinkIcon className="w-3 h-3 text-gray-400 ml-2 flex-shrink-0 opacity-0 group-hover:opacity-100" />
              </button>

              {/* Context after */}
              {showContext && showFullContext && match.contextAfter?.map((ctx, ctxIndex) => (
                <div
                  key={`after-${ctxIndex}`}
                  className="flex items-start px-3 py-1 text-gray-400 dark:text-gray-500"
                >
                  <span className="w-12 flex-shrink-0 text-right pr-3 text-xs font-mono">
                    {match.line + ctxIndex + 1}
                  </span>
                  <pre className="flex-1 text-xs font-mono overflow-x-auto whitespace-pre">
                    {ctx}
                  </pre>
                </div>
              ))}
            </div>
          ))}

          {/* Context toggle */}
          {showContext && group.matches.some(m => m.contextBefore?.length || m.contextAfter?.length) && (
            <button
              onClick={() => setShowFullContext(!showFullContext)}
              className={clsx(
                'w-full px-3 py-1.5 text-xs text-center',
                'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300',
                'hover:bg-gray-50 dark:hover:bg-gray-800',
                'transition-colors'
              )}
            >
              {showFullContext ? 'Hide context lines' : 'Show context lines'}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// GrepResultViewer Component
// ============================================================================

export function GrepResultViewer({
  matches,
  pattern,
  outputMode = 'content',
  files,
  onMatchClick,
  maxHeight = 400,
  className,
}: GrepResultViewerProps) {
  // State
  const [filter, setFilter] = useState('');
  const [copied, setCopied] = useState(false);
  const [expandAll, setExpandAll] = useState(true);

  // Group matches by file
  const groupedMatches = useMemo(() => {
    const groups = new Map<string, GrepMatch[]>();

    matches.forEach(match => {
      if (!groups.has(match.file)) {
        groups.set(match.file, []);
      }
      groups.get(match.file)!.push(match);
    });

    const result: GroupedMatches[] = [];
    groups.forEach((matches, file) => {
      result.push({ file, matches });
    });

    // Sort by file path
    return result.sort((a, b) => a.file.localeCompare(b.file));
  }, [matches]);

  // Filtered results
  const filteredGroups = useMemo(() => {
    if (!filter) return groupedMatches;

    const lowerFilter = filter.toLowerCase();
    return groupedMatches.filter(group =>
      group.file.toLowerCase().includes(lowerFilter) ||
      group.matches.some(m => m.content.toLowerCase().includes(lowerFilter))
    );
  }, [groupedMatches, filter]);

  // Total match count
  const totalMatches = useMemo(() =>
    groupedMatches.reduce((sum, group) => sum + group.matches.length, 0),
    [groupedMatches]
  );

  // Copy results
  const handleCopy = useCallback(async () => {
    const text = filteredGroups.map(group =>
      group.matches.map(m => `${group.file}:${m.line}:${m.content}`).join('\n')
    ).join('\n');

    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, [filteredGroups]);

  // Files-only mode
  if (outputMode === 'files_with_matches' && files) {
    return (
      <div className={clsx('flex flex-col', className)}>
        {/* Header */}
        <div className={clsx(
          'flex items-center justify-between px-3 py-2',
          'bg-gray-50 dark:bg-gray-800/50',
          'border-b border-gray-200 dark:border-gray-700',
          'rounded-t-lg'
        )}>
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
              {files.length} file{files.length !== 1 ? 's' : ''} with matches
            </span>
            {pattern && (
              <code className="text-xs px-1.5 py-0.5 rounded bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400">
                {pattern}
              </code>
            )}
          </div>
        </div>

        {/* File list */}
        <div
          className="overflow-auto divide-y divide-gray-100 dark:divide-gray-800"
          style={{ maxHeight }}
        >
          {files.map((file, index) => {
            const filename = file.split(/[/\\]/).pop() || file;
            const { Icon, color } = getFileIcon(filename);

            return (
              <button
                key={index}
                onClick={() => onMatchClick?.(file)}
                className={clsx(
                  'w-full flex items-center gap-2 px-3 py-2',
                  'hover:bg-gray-50 dark:hover:bg-gray-800',
                  'transition-colors text-left'
                )}
              >
                <Icon className={clsx('w-4 h-4 flex-shrink-0', color)} />
                <span className="text-sm font-mono truncate">
                  {file}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className={clsx('flex flex-col', className)}>
      {/* Header */}
      <div className={clsx(
        'flex items-center justify-between px-3 py-2',
        'bg-gray-50 dark:bg-gray-800/50',
        'border-b border-gray-200 dark:border-gray-700',
        'rounded-t-lg'
      )}>
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {totalMatches} match{totalMatches !== 1 ? 'es' : ''} in {groupedMatches.length} file{groupedMatches.length !== 1 ? 's' : ''}
          </span>
          {pattern && (
            <code className="text-xs px-1.5 py-0.5 rounded bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400">
              {pattern}
            </code>
          )}
        </div>

        <div className="flex items-center gap-1">
          {/* Expand/Collapse all */}
          <button
            onClick={() => setExpandAll(!expandAll)}
            className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
            title={expandAll ? 'Collapse all' : 'Expand all'}
          >
            {expandAll ? (
              <ChevronDownIcon className="w-4 h-4" />
            ) : (
              <ChevronRightIcon className="w-4 h-4" />
            )}
          </button>

          {/* Copy button */}
          <button
            onClick={handleCopy}
            className={clsx(
              'p-1.5 rounded transition-colors',
              copied
                ? 'text-green-500'
                : 'text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
            title={copied ? 'Copied!' : 'Copy results'}
          >
            {copied ? <CheckIcon className="w-4 h-4" /> : <CopyIcon className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Filter input */}
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter results..."
            className={clsx(
              'w-full pl-8 pr-8 py-1.5 rounded text-sm',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          />
          {filter && (
            <button
              onClick={() => setFilter('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
            >
              <Cross2Icon className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Results */}
      <div
        className="overflow-auto p-3 space-y-2"
        style={{ maxHeight }}
      >
        {filteredGroups.map((group, index) => (
          <FileMatchGroup
            key={group.file}
            group={group}
            pattern={pattern}
            onMatchClick={onMatchClick}
            defaultExpanded={expandAll && index < 10}
          />
        ))}

        {filteredGroups.length === 0 && (
          <div className="flex flex-col items-center justify-center py-8 text-gray-500">
            <MagnifyingGlassIcon className="w-8 h-8 mb-2 opacity-50" />
            <p className="text-sm">
              {filter ? 'No results match the filter' : 'No matches found'}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Exports
// ============================================================================

export { highlightPattern };
export default GrepResultViewer;
