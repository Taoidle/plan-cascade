/**
 * DiffViewer Component
 *
 * Shared diff rendering component used by both FileChangeEntry (AI Changes tab)
 * and FileChangeCard (inline chat cards). Renders unified diff output with
 * syntax-colored +/- lines and optional truncation.
 */

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';

interface DiffViewerProps {
  diffContent: string;
  /** Truncate after this many lines. Omit for unlimited. */
  maxLines?: number;
  /** Show "... +N more lines" when truncated. */
  showTruncation?: boolean;
  className?: string;
}

export function DiffViewer({ diffContent, maxLines, showTruncation, className }: DiffViewerProps) {
  const { t } = useTranslation('simpleMode');

  const { visibleLines, truncatedCount } = useMemo(() => {
    const allLines = diffContent.split('\n');
    // Remove trailing empty line from split
    if (allLines.length > 0 && allLines[allLines.length - 1] === '') {
      allLines.pop();
    }
    if (maxLines !== undefined && allLines.length > maxLines) {
      return {
        visibleLines: allLines.slice(0, maxLines),
        truncatedCount: allLines.length - maxLines,
      };
    }
    return { visibleLines: allLines, truncatedCount: 0 };
  }, [diffContent, maxLines]);

  return (
    <pre
      className={clsx(
        'text-2xs font-mono bg-gray-50 dark:bg-gray-800/70 rounded p-2 max-h-64 overflow-auto whitespace-pre-wrap break-all',
        className,
      )}
    >
      {visibleLines.map((line, i) => (
        <div
          key={i}
          className={clsx(
            line.startsWith('+') &&
              'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/20',
            line.startsWith('-') &&
              'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20',
          )}
        >
          {line}
        </div>
      ))}
      {showTruncation && truncatedCount > 0 && (
        <div className="text-gray-400 dark:text-gray-500 italic mt-1">
          {t('workflow.fileChange.moreLines', { count: truncatedCount })}
        </div>
      )}
    </pre>
  );
}
