/**
 * TruncatedText Component
 *
 * Implements intelligent argument preview truncation for tool call parameters
 * with expand/collapse functionality. Shows abbreviated previews for long content
 * with a 'Show more' button to reveal full content.
 *
 * Story-002: Argument preview with truncation and expand functionality
 */

import { useState, useMemo, useCallback, KeyboardEvent } from 'react';
import { clsx } from 'clsx';
import { ChevronDownIcon, ChevronUpIcon, CopyIcon, CheckIcon } from '@radix-ui/react-icons';

// ============================================================================
// Types
// ============================================================================

interface TruncatedTextProps {
  /** The text content to display */
  content: string;
  /** Maximum length before truncation (default: 200) */
  maxLength?: number;
  /** Whether this is a file path (uses path-specific truncation) */
  isPath?: boolean;
  /** Whether this is a command (uses command-specific truncation) */
  isCommand?: boolean;
  /** Whether this is JSON/object content */
  isJson?: boolean;
  /** Maximum height when expanded (default: 400px) */
  maxExpandedHeight?: number;
  /** Additional CSS classes */
  className?: string;
  /** Whether to show the copy button */
  showCopy?: boolean;
  /** Custom label for the content type */
  label?: string;
}

// ============================================================================
// Custom Hook: useExpandable
// ============================================================================

interface UseExpandableOptions {
  defaultExpanded?: boolean;
}

function useExpandable(options: UseExpandableOptions = {}) {
  const [isExpanded, setIsExpanded] = useState(options.defaultExpanded ?? false);

  const toggle = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  const expand = useCallback(() => {
    setIsExpanded(true);
  }, []);

  const collapse = useCallback(() => {
    setIsExpanded(false);
  }, []);

  return {
    isExpanded,
    toggle,
    expand,
    collapse,
    setIsExpanded,
  };
}

// ============================================================================
// Truncation Utilities
// ============================================================================

/**
 * Truncate text at word boundaries where possible
 */
function truncateAtWordBoundary(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;

  // Find the last space within the limit
  const truncatePoint = text.lastIndexOf(' ', maxLength - 3);

  // If no space found or too close to start, just cut at maxLength
  if (truncatePoint < maxLength * 0.5) {
    return text.slice(0, maxLength - 3) + '...';
  }

  return text.slice(0, truncatePoint) + '...';
}

/**
 * Truncate file path while preserving important segments
 * Format: .../parent/filename.ext
 */
function truncatePath(path: string, maxLength: number = 60): string {
  if (path.length <= maxLength) return path;

  const parts = path.split(/[/\\]/);
  const filename = parts.pop() || '';

  // If filename alone is too long, truncate it
  if (filename.length > maxLength - 4) {
    const ext = filename.includes('.') ? '.' + filename.split('.').pop() : '';
    const nameWithoutExt = filename.slice(0, filename.length - ext.length);
    const availableLength = maxLength - 4 - ext.length;
    return '...' + nameWithoutExt.slice(-availableLength) + ext;
  }

  // Build path from end until we exceed limit
  let result = filename;
  for (let i = parts.length - 1; i >= 0; i--) {
    const newResult = parts[i] + '/' + result;
    if (newResult.length > maxLength - 4) {
      return '.../' + result;
    }
    result = newResult;
  }

  return result;
}

/**
 * Truncate command while preserving structure
 */
function truncateCommand(command: string, maxLength: number = 80): string {
  if (command.length <= maxLength) return command;

  // Try to break at pipe or semicolon
  const pipeIndex = command.indexOf('|');
  const semiIndex = command.indexOf(';');
  const andIndex = command.indexOf('&&');

  const breakPoints = [pipeIndex, semiIndex, andIndex].filter((i) => i > 0 && i < maxLength - 3);

  if (breakPoints.length > 0) {
    const breakAt = Math.max(...breakPoints);
    return command.slice(0, breakAt) + '...';
  }

  return command.slice(0, maxLength - 3) + '...';
}

/**
 * Format JSON with depth indicator for collapsed nested objects
 */
function formatJsonPreview(
  content: string,
  maxLength: number = 200,
): {
  preview: string;
  isComplex: boolean;
  depth: number;
} {
  try {
    const parsed = JSON.parse(content);

    // Calculate nesting depth
    const getDepth = (obj: unknown, currentDepth = 0): number => {
      if (typeof obj !== 'object' || obj === null) return currentDepth;
      if (Array.isArray(obj)) {
        return Math.max(currentDepth, ...obj.map((item) => getDepth(item, currentDepth + 1)));
      }
      return Math.max(currentDepth, ...Object.values(obj).map((val) => getDepth(val, currentDepth + 1)));
    };

    const depth = getDepth(parsed);
    const isComplex = depth > 2 || content.length > maxLength;

    if (!isComplex) {
      return { preview: content, isComplex: false, depth };
    }

    // Create simplified preview
    const simplify = (obj: unknown, currentDepth = 0): unknown => {
      if (currentDepth >= 2) {
        if (Array.isArray(obj)) return `[...${obj.length} items]`;
        if (typeof obj === 'object' && obj !== null) {
          return `{...${Object.keys(obj).length} keys}`;
        }
      }
      if (typeof obj !== 'object' || obj === null) return obj;
      if (Array.isArray(obj)) {
        return obj
          .slice(0, 3)
          .map((item) => simplify(item, currentDepth + 1))
          .concat(obj.length > 3 ? [`...+${obj.length - 3} more`] : []);
      }
      const entries = Object.entries(obj);
      const simplified: Record<string, unknown> = {};
      entries.slice(0, 5).forEach(([key, val]) => {
        simplified[key] = simplify(val, currentDepth + 1);
      });
      if (entries.length > 5) {
        simplified['...'] = `+${entries.length - 5} more keys`;
      }
      return simplified;
    };

    const preview = JSON.stringify(simplify(parsed), null, 2);
    return { preview, isComplex: true, depth };
  } catch {
    // Not valid JSON, treat as plain text
    return {
      preview: truncateAtWordBoundary(content, maxLength),
      isComplex: false,
      depth: 0,
    };
  }
}

// ============================================================================
// TruncatedText Component
// ============================================================================

export function TruncatedText({
  content,
  maxLength = 200,
  isPath = false,
  isCommand = false,
  isJson = false,
  maxExpandedHeight = 400,
  className,
  showCopy = true,
  label,
}: TruncatedTextProps) {
  const { isExpanded, toggle } = useExpandable();
  const [copied, setCopied] = useState(false);

  // Calculate truncated content
  const { truncatedContent, needsTruncation, extraInfo } = useMemo(() => {
    if (!content) {
      return { truncatedContent: '', needsTruncation: false, extraInfo: '' };
    }

    if (isPath) {
      const truncated = truncatePath(content, maxLength);
      return {
        truncatedContent: truncated,
        needsTruncation: content.length > maxLength,
        extraInfo: content.length > maxLength ? `(${content.length} chars)` : '',
      };
    }

    if (isCommand) {
      const truncated = truncateCommand(content, maxLength);
      return {
        truncatedContent: truncated,
        needsTruncation: content.length > maxLength,
        extraInfo: content.length > maxLength ? `(${content.length} chars)` : '',
      };
    }

    if (isJson) {
      const { preview, isComplex, depth } = formatJsonPreview(content, maxLength);
      return {
        truncatedContent: preview,
        needsTruncation: isComplex,
        extraInfo: isComplex ? `(depth: ${depth})` : '',
      };
    }

    const truncated = truncateAtWordBoundary(content, maxLength);
    return {
      truncatedContent: truncated,
      needsTruncation: content.length > maxLength,
      extraInfo: content.length > maxLength ? `(${content.length} chars)` : '',
    };
  }, [content, maxLength, isPath, isCommand, isJson]);

  // Copy handler
  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  }, [content]);

  // Keyboard handler for expand/collapse
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        toggle();
      }
    },
    [toggle],
  );

  if (!content) {
    return <span className={clsx('text-gray-400 italic', className)}>No content</span>;
  }

  return (
    <div className={clsx('group relative', className)}>
      {/* Label if provided */}
      {label && (
        <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1 block">
          {label}
        </span>
      )}

      {/* Content container */}
      <div
        className={clsx(
          'relative rounded-lg transition-all duration-200',
          isExpanded ? 'bg-gray-50 dark:bg-gray-800/50' : '',
        )}
      >
        {/* Main content */}
        <div
          className={clsx(
            'font-mono text-sm',
            isPath && 'text-blue-600 dark:text-blue-400',
            isCommand && 'text-purple-600 dark:text-purple-400',
            isJson && 'text-gray-700 dark:text-gray-300',
          )}
        >
          {isExpanded ? (
            <div className="overflow-auto p-3" style={{ maxHeight: `${maxExpandedHeight}px` }}>
              <pre className="whitespace-pre-wrap break-words">
                {isJson ? <code>{JSON.stringify(JSON.parse(content), null, 2)}</code> : content}
              </pre>
            </div>
          ) : (
            <code
              className={clsx(
                'block px-2 py-1 rounded',
                'bg-gray-100 dark:bg-gray-800',
                needsTruncation && 'cursor-pointer hover:bg-gray-200 dark:hover:bg-gray-700',
              )}
            >
              {truncatedContent}
            </code>
          )}
        </div>

        {/* Controls */}
        {(needsTruncation || showCopy) && (
          <div className={clsx('flex items-center gap-2 mt-2', 'text-xs text-gray-500 dark:text-gray-400')}>
            {/* Expand/Collapse button */}
            {needsTruncation && (
              <button
                onClick={toggle}
                onKeyDown={handleKeyDown}
                className={clsx(
                  'flex items-center gap-1 px-2 py-1 rounded',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
                aria-expanded={isExpanded}
                aria-label={isExpanded ? 'Show less' : 'Show more'}
              >
                {isExpanded ? (
                  <>
                    <ChevronUpIcon className="w-3 h-3" />
                    <span>Show less</span>
                  </>
                ) : (
                  <>
                    <ChevronDownIcon className="w-3 h-3" />
                    <span>Show more {extraInfo}</span>
                  </>
                )}
              </button>
            )}

            {/* Copy button */}
            {showCopy && (
              <button
                onClick={handleCopy}
                className={clsx(
                  'flex items-center gap-1 px-2 py-1 rounded',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500',
                  copied && 'text-green-600 dark:text-green-400',
                )}
                aria-label={copied ? 'Copied!' : 'Copy to clipboard'}
              >
                {copied ? (
                  <>
                    <CheckIcon className="w-3 h-3" />
                    <span>Copied!</span>
                  </>
                ) : (
                  <>
                    <CopyIcon className="w-3 h-3" />
                    <span>Copy</span>
                  </>
                )}
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Exports
// ============================================================================

export { useExpandable, truncatePath, truncateCommand, truncateAtWordBoundary };
export default TruncatedText;
