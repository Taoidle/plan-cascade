/**
 * ConflictResolver Component
 *
 * Full-panel overlay shown during merge conflicts.
 * Lists conflicting files, tracks resolution progress,
 * and allows completing or aborting the merge.
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 */

import { useState, useCallback, useEffect } from 'react';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';
import { useSettingsStore } from '../../../../store/settings';
import { ThreeWayDiff } from './ThreeWayDiff';
import type { ConflictFile } from '../../../../types/git';

// ---------------------------------------------------------------------------
// ConflictFileRow
// ---------------------------------------------------------------------------

function ConflictFileRow({
  file,
  isResolved,
  isSelected,
  onClick,
}: {
  file: ConflictFile;
  isResolved: boolean;
  isSelected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        'w-full flex items-center gap-2 px-3 py-2 text-left transition-colors rounded-lg',
        isSelected
          ? 'bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800'
          : 'hover:bg-gray-50 dark:hover:bg-gray-800/50 border border-transparent'
      )}
    >
      {/* Status icon */}
      <span
        className={clsx(
          'shrink-0 w-5 h-5 flex items-center justify-center rounded-full text-xs font-bold',
          isResolved
            ? 'bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400'
            : 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400'
        )}
      >
        {isResolved ? '\u2713' : '\u2717'}
      </span>

      {/* File path */}
      <span className="flex-1 text-sm text-gray-800 dark:text-gray-200 truncate font-mono">
        {file.path}
      </span>

      {/* Conflict count */}
      <span className="shrink-0 text-2xs text-gray-500 dark:text-gray-400">
        {file.conflict_count} {file.conflict_count === 1 ? 'conflict' : 'conflicts'}
      </span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// ConflictResolver Component
// ---------------------------------------------------------------------------

export function ConflictResolver() {
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const {
    conflictFiles,
    resolvedFiles,
    mergeSourceBranch,
    isInMerge,
    abortMerge,
    completeMerge,
    refreshConflictFiles,
    markFileResolved,
  } = useGitStore();

  const [selectedFile, setSelectedFile] = useState<ConflictFile | null>(null);
  const [isAborting, setIsAborting] = useState(false);
  const [isCompleting, setIsCompleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const repoPath = workspacePath || '';

  // Auto-select first unresolved file
  useEffect(() => {
    if (conflictFiles.length > 0 && !selectedFile) {
      const firstUnresolved = conflictFiles.find((f) => !resolvedFiles.has(f.path));
      setSelectedFile(firstUnresolved || conflictFiles[0]);
    }
  }, [conflictFiles, selectedFile, resolvedFiles]);

  // Refresh conflicts periodically
  useEffect(() => {
    if (repoPath && isInMerge) {
      refreshConflictFiles(repoPath);
    }
  }, [repoPath, isInMerge, refreshConflictFiles]);

  const resolvedCount = conflictFiles.filter((f) => resolvedFiles.has(f.path)).length;
  const totalCount = conflictFiles.length;
  const allResolved = resolvedCount === totalCount && totalCount > 0;

  const handleAbort = useCallback(async () => {
    setIsAborting(true);
    setError(null);
    try {
      const ok = await abortMerge(repoPath);
      if (!ok) {
        setError('Failed to abort merge');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsAborting(false);
    }
  }, [repoPath, abortMerge]);

  const handleComplete = useCallback(async () => {
    if (!allResolved) return;
    setIsCompleting(true);
    setError(null);
    try {
      const ok = await completeMerge(repoPath);
      if (!ok) {
        setError('Failed to complete merge. Ensure all conflicts are resolved and files are staged.');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCompleting(false);
    }
  }, [repoPath, allResolved, completeMerge]);

  const handleFileResolved = useCallback(
    (filePath: string) => {
      markFileResolved(filePath);
      // Auto-advance to next unresolved file
      const nextUnresolved = conflictFiles.find(
        (f) => f.path !== filePath && !resolvedFiles.has(f.path)
      );
      if (nextUnresolved) {
        setSelectedFile(nextUnresolved);
      }
    },
    [conflictFiles, resolvedFiles, markFileResolved]
  );

  if (!isInMerge) return null;

  return (
    <div className="fixed inset-0 z-40 bg-white dark:bg-gray-900 flex flex-col">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            <svg className="w-5 h-5 text-amber-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
            </svg>
            <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
              Merge Conflict Resolution
            </h2>
          </div>
          {mergeSourceBranch && (
            <span className="text-sm text-gray-500 dark:text-gray-400">
              Merging <code className="px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-xs font-mono">{mergeSourceBranch}</code>
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          {/* Progress */}
          <span className="text-sm text-gray-600 dark:text-gray-400">
            <span className={clsx(allResolved ? 'text-green-600 dark:text-green-400 font-medium' : '')}>
              {resolvedCount} of {totalCount}
            </span>
            {' '}conflicts resolved
          </span>

          {/* Abort */}
          <button
            onClick={handleAbort}
            disabled={isAborting}
            className={clsx(
              'px-3 py-1.5 text-sm rounded-lg border transition-colors',
              'border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              isAborting && 'opacity-50 cursor-not-allowed'
            )}
          >
            {isAborting ? 'Aborting...' : 'Abort Merge'}
          </button>

          {/* Complete */}
          <button
            onClick={handleComplete}
            disabled={!allResolved || isCompleting}
            className={clsx(
              'px-3 py-1.5 text-sm rounded-lg font-medium text-white transition-colors',
              allResolved && !isCompleting
                ? 'bg-green-600 hover:bg-green-700'
                : 'bg-green-400 cursor-not-allowed'
            )}
          >
            {isCompleting ? 'Completing...' : 'Complete Merge'}
          </button>
        </div>
      </div>

      {/* Error bar */}
      {error && (
        <div className="shrink-0 px-4 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-600 dark:text-red-400 border-b border-red-200 dark:border-red-800">
          {error}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 min-h-0 flex">
        {/* File list sidebar */}
        <div className="w-72 shrink-0 border-r border-gray-200 dark:border-gray-700 overflow-y-auto p-2 space-y-1">
          {conflictFiles.map((file) => (
            <ConflictFileRow
              key={file.path}
              file={file}
              isResolved={resolvedFiles.has(file.path)}
              isSelected={selectedFile?.path === file.path}
              onClick={() => setSelectedFile(file)}
            />
          ))}
          {conflictFiles.length === 0 && (
            <div className="py-8 text-center text-sm text-gray-500 dark:text-gray-400">
              No conflict files detected
            </div>
          )}
        </div>

        {/* Diff view */}
        <div className="flex-1 min-w-0 overflow-hidden">
          {selectedFile ? (
            <ThreeWayDiff
              repoPath={repoPath}
              filePath={selectedFile.path}
              onResolved={() => handleFileResolved(selectedFile.path)}
            />
          ) : (
            <div className="flex items-center justify-center h-full text-sm text-gray-500 dark:text-gray-400">
              Select a file to resolve conflicts
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default ConflictResolver;
