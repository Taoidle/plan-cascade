/**
 * FileChangeEntry Component
 *
 * A single file change row within a turn group. Shows the tool type,
 * file path, and description. Expandable to show inline diff.
 */

import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { ChevronRightIcon, ChevronDownIcon } from '@radix-ui/react-icons';
import { useFileChangesStore } from '../../../../store/fileChanges';
import type { FileChange } from '../../../../types/fileChanges';

interface FileChangeEntryProps {
  change: FileChange;
  sessionId: string;
  projectRoot: string;
}

export function FileChangeEntry({ change, sessionId, projectRoot }: FileChangeEntryProps) {
  const { t } = useTranslation('git');
  const expanded = useFileChangesStore((s) => s.expandedChangeIds.has(change.id));
  const toggleExpanded = useFileChangesStore((s) => s.toggleExpanded);
  const fetchDiff = useFileChangesStore((s) => s.fetchDiff);
  const diffCache = useFileChangesStore((s) => s.diffCache);

  const [diffContent, setDiffContent] = useState<string | null>(null);
  const [loadingDiff, setLoadingDiff] = useState(false);

  const handleToggle = useCallback(() => {
    toggleExpanded(change.id);
  }, [change.id, toggleExpanded]);

  // Load diff when expanded
  useEffect(() => {
    if (!expanded) return;
    const cached = diffCache.get(change.id);
    if (cached !== undefined) {
      setDiffContent(cached);
      return;
    }
    setLoadingDiff(true);
    fetchDiff(sessionId, projectRoot, change.id, change.before_hash, change.after_hash).then(
      (diff) => {
        setDiffContent(diff);
        setLoadingDiff(false);
      },
    );
  }, [expanded, change, sessionId, projectRoot, fetchDiff, diffCache]);

  return (
    <div className="border-t border-gray-100 dark:border-gray-800 first:border-t-0">
      {/* Header row */}
      <button
        onClick={handleToggle}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
      >
        {expanded ? (
          <ChevronDownIcon className="w-3 h-3 text-gray-400 shrink-0" />
        ) : (
          <ChevronRightIcon className="w-3 h-3 text-gray-400 shrink-0" />
        )}

        {/* Tool badge */}
        <span
          className={clsx(
            'inline-block min-w-[2.5rem] text-center rounded px-1 py-0.5 text-2xs font-medium shrink-0',
            change.tool_name === 'Write'
              ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
              : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
          )}
        >
          {change.tool_name}
        </span>

        {/* File path */}
        <span className="text-xs font-mono text-gray-700 dark:text-gray-300 truncate flex-1">
          {change.file_path}
        </span>

        {/* Description (short) */}
        <span className="text-2xs text-gray-400 dark:text-gray-500 shrink-0 hidden sm:inline">
          {change.description}
        </span>
      </button>

      {/* Expanded diff content */}
      {expanded && (
        <div className="px-3 pb-2">
          {loadingDiff ? (
            <p className="text-2xs text-gray-400 py-2">{t('aiChanges.loadingDiff')}</p>
          ) : diffContent ? (
            <pre className="text-2xs font-mono bg-gray-50 dark:bg-gray-800/70 rounded p-2 max-h-64 overflow-auto whitespace-pre-wrap break-all">
              {diffContent.split('\n').map((line, i) => (
                <div
                  key={i}
                  className={clsx(
                    line.startsWith('+') && 'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/20',
                    line.startsWith('-') && 'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20',
                  )}
                >
                  {line}
                </div>
              ))}
            </pre>
          ) : (
            <p className="text-2xs text-gray-400 py-2">{t('aiChanges.noDiff')}</p>
          )}
        </div>
      )}
    </div>
  );
}
