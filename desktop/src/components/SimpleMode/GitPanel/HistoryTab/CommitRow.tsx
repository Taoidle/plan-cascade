/**
 * CommitRow Component
 *
 * A single row (36px height) in the commit history graph showing:
 * - Ref badges (branch name pills colored to match graph lane, tag badges in yellow)
 * - Commit message (truncated with ellipsis)
 * - Author name (abbreviated)
 * - Relative time (e.g., "2h ago", "3 days ago")
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import type { CommitNode } from '../../../../types/git';
import { ROW_HEIGHT } from './graphRenderer';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CommitRowProps {
  /** The commit data */
  commit: CommitNode;
  /** Whether this row is selected */
  isSelected: boolean;
  /** Whether this row is the compare target (shift+click) */
  isCompareTarget: boolean;
  /** Whether this commit is the current HEAD */
  isHead: boolean;
  /** Lane color for ref badges on this commit */
  laneColor: string;
  /** Search query for highlighting matches */
  searchQuery: string;
  /** Click handler */
  onClick: (e: React.MouseEvent) => void;
  /** Context menu handler */
  onContextMenu: (e: React.MouseEvent) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Format an ISO-8601 date string as a relative time.
 * Returns strings like "just now", "2m ago", "3h ago", "5d ago", "2w ago", etc.
 */
function formatRelativeTime(isoDate: string): string {
  const date = new Date(isoDate);
  const now = Date.now();
  const diffMs = now - date.getTime();

  if (diffMs < 0) return 'in the future';

  const seconds = Math.floor(diffMs / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const weeks = Math.floor(days / 7);
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);

  if (seconds < 60) return 'just now';
  if (minutes < 60) return `${minutes}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;
  if (weeks < 5) return `${weeks}w ago`;
  if (months < 12) return `${months}mo ago`;
  return `${years}y ago`;
}

/**
 * Abbreviate an author name.
 * "John Doe" -> "J. Doe"
 * "alice" -> "alice"
 */
function abbreviateAuthor(name: string): string {
  const parts = name.trim().split(/\s+/);
  if (parts.length <= 1) return name;
  return `${parts[0][0]}. ${parts.slice(1).join(' ')}`;
}

/**
 * Parse ref names to separate branches and tags.
 * Ref format from git: "HEAD -> main", "origin/main", "tag: v1.0"
 */
interface ParsedRefs {
  branches: { name: string; isHead: boolean }[];
  tags: string[];
}

function parseRefs(refs: string[]): ParsedRefs {
  const branches: { name: string; isHead: boolean }[] = [];
  const tags: string[] = [];

  for (const ref of refs) {
    const trimmed = ref.trim();
    if (!trimmed) continue;

    if (trimmed.startsWith('tag: ')) {
      tags.push(trimmed.replace('tag: ', ''));
    } else if (trimmed.startsWith('HEAD -> ')) {
      branches.push({ name: trimmed.replace('HEAD -> ', ''), isHead: true });
    } else if (trimmed === 'HEAD') {
      // Detached HEAD, skip
      continue;
    } else {
      branches.push({ name: trimmed, isHead: false });
    }
  }

  return { branches, tags };
}

/**
 * Get the first line of a commit message (the subject).
 */
function commitSubject(message: string): string {
  const firstLine = message.split('\n')[0];
  return firstLine.trim();
}

/**
 * Highlight search matches in text.
 * Returns an array of spans with matched portions highlighted.
 */
function highlightMatch(
  text: string,
  query: string
): { text: string; isMatch: boolean }[] {
  if (!query) return [{ text, isMatch: false }];

  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const parts: { text: string; isMatch: boolean }[] = [];
  let lastIndex = 0;

  let index = lowerText.indexOf(lowerQuery, lastIndex);
  while (index !== -1) {
    if (index > lastIndex) {
      parts.push({ text: text.slice(lastIndex, index), isMatch: false });
    }
    parts.push({ text: text.slice(index, index + query.length), isMatch: true });
    lastIndex = index + query.length;
    index = lowerText.indexOf(lowerQuery, lastIndex);
  }

  if (lastIndex < text.length) {
    parts.push({ text: text.slice(lastIndex), isMatch: false });
  }

  return parts.length > 0 ? parts : [{ text, isMatch: false }];
}

// ---------------------------------------------------------------------------
// CommitRow Component
// ---------------------------------------------------------------------------

export function CommitRow({
  commit,
  isSelected,
  isCompareTarget,
  isHead,
  laneColor,
  searchQuery,
  onClick,
  onContextMenu,
}: CommitRowProps) {
  const subject = useMemo(() => commitSubject(commit.message), [commit.message]);
  const { branches, tags } = useMemo(() => parseRefs(commit.refs), [commit.refs]);
  const relativeTime = useMemo(() => formatRelativeTime(commit.date), [commit.date]);
  const authorAbbr = useMemo(() => abbreviateAuthor(commit.author_name), [commit.author_name]);

  const highlightedSubject = useMemo(
    () => highlightMatch(subject, searchQuery),
    [subject, searchQuery]
  );

  return (
    <div
      className={clsx(
        'flex items-center gap-2 px-2 cursor-pointer select-none border-b border-transparent transition-colors',
        isSelected && 'bg-blue-50 dark:bg-blue-900/30 border-b-blue-200 dark:border-b-blue-800',
        isCompareTarget && !isSelected && 'bg-purple-50 dark:bg-purple-900/20 border-b-purple-200 dark:border-b-purple-800',
        !isSelected && !isCompareTarget && 'hover:bg-gray-50 dark:hover:bg-gray-800/50',
      )}
      style={{ height: ROW_HEIGHT }}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(e);
      }}
      title={`${commit.short_sha} - ${subject}`}
    >
      {/* HEAD indicator */}
      {isHead && (
        <div
          className="shrink-0 w-1.5 h-1.5 rounded-full"
          style={{ backgroundColor: laneColor }}
        />
      )}

      {/* Ref badges */}
      {branches.length > 0 && (
        <div className="shrink-0 flex items-center gap-1">
          {branches.slice(0, 2).map((branch) => (
            <span
              key={branch.name}
              className={clsx(
                'inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium leading-none truncate max-w-[100px]',
                branch.isHead
                  ? 'text-white'
                  : 'text-white/90',
              )}
              style={{ backgroundColor: laneColor }}
              title={branch.name}
            >
              {branch.name}
            </span>
          ))}
          {branches.length > 2 && (
            <span
              className="text-[10px] text-gray-500 dark:text-gray-400"
              title={branches.map((b) => b.name).join(', ')}
            >
              +{branches.length - 2}
            </span>
          )}
        </div>
      )}

      {tags.length > 0 && (
        <div className="shrink-0 flex items-center gap-1">
          {tags.slice(0, 1).map((tag) => (
            <span
              key={tag}
              className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium leading-none bg-yellow-100 dark:bg-yellow-900/40 text-yellow-800 dark:text-yellow-300 truncate max-w-[80px]"
              title={tag}
            >
              {tag}
            </span>
          ))}
          {tags.length > 1 && (
            <span className="text-[10px] text-gray-500 dark:text-gray-400">
              +{tags.length - 1}
            </span>
          )}
        </div>
      )}

      {/* Commit message */}
      <span className="flex-1 min-w-0 text-xs text-gray-800 dark:text-gray-200 truncate">
        {highlightedSubject.map((part, i) =>
          part.isMatch ? (
            <mark
              key={i}
              className="bg-yellow-200 dark:bg-yellow-700/60 text-gray-900 dark:text-yellow-100 rounded-sm px-0.5"
            >
              {part.text}
            </mark>
          ) : (
            <span key={i}>{part.text}</span>
          )
        )}
      </span>

      {/* Author */}
      <span className="shrink-0 text-[10px] text-gray-500 dark:text-gray-400 max-w-[80px] truncate">
        {authorAbbr}
      </span>

      {/* Relative time */}
      <span className="shrink-0 text-[10px] text-gray-400 dark:text-gray-500 w-[52px] text-right">
        {relativeTime}
      </span>
    </div>
  );
}

export default CommitRow;
