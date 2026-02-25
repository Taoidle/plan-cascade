/**
 * DiffViewer Component
 *
 * Displays unified diffs between checkpoints with syntax highlighting.
 * Shows file-level changes with added/removed lines colored appropriately.
 */

import { useState, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { FileIcon, PlusIcon, MinusIcon, ChevronDownIcon, ChevronUpIcon } from '@radix-ui/react-icons';
import type { CheckpointDiff, FileDiff } from '../../types/timeline';

interface DiffViewerProps {
  diff: CheckpointDiff;
  className?: string;
}

// File type to language mapping for basic syntax hints
const FILE_TYPES: Record<string, string> = {
  js: 'javascript',
  jsx: 'javascript',
  ts: 'typescript',
  tsx: 'typescript',
  rs: 'rust',
  py: 'python',
  json: 'json',
  md: 'markdown',
  html: 'html',
  css: 'css',
  scss: 'scss',
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'toml',
};

// Helper function to get file extension (used by FILE_TYPES mapping)
function _getFileExtension(path: string): string {
  const parts = path.split('.');
  return parts.length > 1 ? parts[parts.length - 1].toLowerCase() : '';
}

// Get language identifier for syntax highlighting (available for future use)
export function getFileLanguage(path: string): string {
  const ext = _getFileExtension(path);
  return FILE_TYPES[ext] || 'text';
}

interface FileItemProps {
  file: FileDiff;
  defaultExpanded?: boolean;
}

function FileItem({ file, defaultExpanded = false }: FileItemProps) {
  const { t } = useTranslation();
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  // Parse diff content into lines
  const diffLines = useMemo(() => {
    if (!file.diff_content) return [];

    return file.diff_content.split('\n').map((line, idx) => {
      let type: 'added' | 'removed' | 'context' | 'header' = 'context';
      if (line.startsWith('+++') || line.startsWith('---') || line.startsWith('@@')) {
        type = 'header';
      } else if (line.startsWith('+')) {
        type = 'added';
      } else if (line.startsWith('-')) {
        type = 'removed';
      }
      return { line, type, key: idx };
    });
  }, [file.diff_content]);

  // Get change type badge color
  const getBadgeColor = (changeType: string) => {
    switch (changeType) {
      case 'added':
        return 'bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-300';
      case 'modified':
        return 'bg-yellow-100 dark:bg-yellow-900/50 text-yellow-700 dark:text-yellow-300';
      case 'deleted':
        return 'bg-red-100 dark:bg-red-900/50 text-red-700 dark:text-red-300';
      default:
        return 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300';
    }
  };

  return (
    <div className={clsx('border rounded-lg overflow-hidden', 'border-gray-200 dark:border-gray-700')}>
      {/* File header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={clsx(
          'w-full flex items-center justify-between px-3 py-2',
          'bg-gray-50 dark:bg-gray-800',
          'hover:bg-gray-100 dark:hover:bg-gray-700',
          'transition-colors',
        )}
      >
        <div className="flex items-center gap-2 min-w-0">
          <FileIcon className="w-4 h-4 text-gray-400 flex-shrink-0" />
          <span className="text-sm font-medium text-gray-900 dark:text-white truncate">{file.path}</span>
          <span className={clsx('px-2 py-0.5 text-xs font-medium rounded', getBadgeColor(file.change_type))}>
            {t(`timeline.${file.change_type}`)}
          </span>
          {file.is_binary && (
            <span className="px-2 py-0.5 text-xs font-medium rounded bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400">
              binary
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          {/* Line stats */}
          {(file.lines_added > 0 || file.lines_removed > 0) && (
            <div className="flex items-center gap-2 text-xs">
              {file.lines_added > 0 && (
                <span className="text-green-600 dark:text-green-400 flex items-center gap-0.5">
                  <PlusIcon className="w-3 h-3" />
                  {file.lines_added}
                </span>
              )}
              {file.lines_removed > 0 && (
                <span className="text-red-600 dark:text-red-400 flex items-center gap-0.5">
                  <MinusIcon className="w-3 h-3" />
                  {file.lines_removed}
                </span>
              )}
            </div>
          )}

          {isExpanded ? (
            <ChevronUpIcon className="w-4 h-4 text-gray-400" />
          ) : (
            <ChevronDownIcon className="w-4 h-4 text-gray-400" />
          )}
        </div>
      </button>

      {/* Diff content */}
      {isExpanded && (
        <div className="overflow-x-auto">
          {file.is_binary ? (
            <div className="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 italic">
              Binary file - content not displayed
            </div>
          ) : diffLines.length > 0 ? (
            <pre className="text-sm font-mono">
              {diffLines.map(({ line, type, key }) => (
                <div
                  key={key}
                  className={clsx(
                    'px-4 py-0.5',
                    type === 'added' && 'bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-300',
                    type === 'removed' && 'bg-red-50 dark:bg-red-900/20 text-red-800 dark:text-red-300',
                    type === 'header' && 'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 font-medium',
                    type === 'context' && 'text-gray-600 dark:text-gray-400',
                  )}
                >
                  {line || '\u00A0'}
                </div>
              ))}
            </pre>
          ) : (
            <div className="px-4 py-3 text-sm text-gray-500 dark:text-gray-400 italic">{t('timeline.noDiff')}</div>
          )}
        </div>
      )}
    </div>
  );
}

export function DiffViewer({ diff, className }: DiffViewerProps) {
  const { t } = useTranslation();

  // Combine all files in order: added, modified, deleted
  const allFiles = useMemo(() => {
    return [...diff.added_files, ...diff.modified_files, ...diff.deleted_files];
  }, [diff]);

  if (allFiles.length === 0) {
    return (
      <div className={clsx('text-center py-8', className)}>
        <p className="text-gray-500 dark:text-gray-400">{t('timeline.noDiff')}</p>
      </div>
    );
  }

  return (
    <div className={clsx('space-y-4', className)}>
      {/* Summary */}
      <div className="flex items-center gap-4 text-sm">
        <span className="font-medium text-gray-900 dark:text-white">{t('timeline.diffTitle')}:</span>
        <div className="flex items-center gap-3">
          {diff.summary.files_added > 0 && (
            <span className="text-green-600 dark:text-green-400">
              +{diff.summary.files_added} {t('timeline.added')}
            </span>
          )}
          {diff.summary.files_modified > 0 && (
            <span className="text-yellow-600 dark:text-yellow-400">
              ~{diff.summary.files_modified} {t('timeline.modified')}
            </span>
          )}
          {diff.summary.files_deleted > 0 && (
            <span className="text-red-600 dark:text-red-400">
              -{diff.summary.files_deleted} {t('timeline.deleted')}
            </span>
          )}
        </div>
        <span className="text-gray-400">|</span>
        <span className="text-green-600 dark:text-green-400">
          +{diff.summary.lines_added} {t('timeline.linesAdded')}
        </span>
        <span className="text-red-600 dark:text-red-400">
          -{diff.summary.lines_removed} {t('timeline.linesRemoved')}
        </span>
      </div>

      {/* File list */}
      <div className="space-y-2">
        {allFiles.map((file, idx) => (
          <FileItem key={file.path} file={file} defaultExpanded={idx === 0} />
        ))}
      </div>
    </div>
  );
}

export default DiffViewer;
