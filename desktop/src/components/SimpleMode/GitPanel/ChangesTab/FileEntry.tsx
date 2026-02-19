/**
 * FileEntry Component
 *
 * Single file row in the staging area. Shows change type badge, file path,
 * and stage/unstage toggle button. Clicking expands an inline diff view
 * with per-hunk stage/unstage buttons using EnhancedDiffViewer.
 *
 * Feature-002, Story-004 + Story-007 (hunk-level staging)
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore, type FileStatus, type DiffOutput, type DiffHunk } from '../../../../store/git';
import { EnhancedDiffViewer } from '../../../ClaudeCodeMode/EnhancedDiffViewer';

// ============================================================================
// Types
// ============================================================================

interface FileEntryProps {
  file: FileStatus;
  /** Whether this file is currently in the staged area. */
  isStaged: boolean;
  /** Whether this file is in the untracked section. */
  isUntracked?: boolean;
}

// ============================================================================
// Helpers
// ============================================================================

/** Map FileStatusKind to a display badge letter and color. */
function getBadgeInfo(kind: FileStatus['kind']): { letter: string; className: string } {
  switch (kind) {
    case 'added':
      return { letter: 'A', className: 'bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-400' };
    case 'modified':
      return { letter: 'M', className: 'bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-400' };
    case 'deleted':
      return { letter: 'D', className: 'bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-400' };
    case 'renamed':
      return { letter: 'R', className: 'bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-400' };
    case 'copied':
      return { letter: 'C', className: 'bg-cyan-100 dark:bg-cyan-900/40 text-cyan-700 dark:text-cyan-400' };
    case 'untracked':
      return { letter: 'U', className: 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400' };
    case 'unmerged':
      return { letter: '!', className: 'bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-400' };
    case 'type_changed':
      return { letter: 'T', className: 'bg-yellow-100 dark:bg-yellow-900/40 text-yellow-700 dark:text-yellow-400' };
    default:
      return { letter: '?', className: 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400' };
  }
}

/** Convert a single hunk into old/new content strings for EnhancedDiffViewer. */
function hunkToContents(hunk: DiffHunk): { oldContent: string; newContent: string } {
  const oldLines: string[] = [];
  const newLines: string[] = [];

  for (const line of hunk.lines) {
    switch (line.kind) {
      case 'context':
        oldLines.push(line.content);
        newLines.push(line.content);
        break;
      case 'deletion':
        oldLines.push(line.content);
        break;
      case 'addition':
        newLines.push(line.content);
        break;
      // hunk_header is metadata, skip
    }
  }

  return {
    oldContent: oldLines.join('\n'),
    newContent: newLines.join('\n'),
  };
}

/** Convert all hunks of a DiffOutput into old/new content strings. */
function diffOutputToContents(diff: DiffOutput, filePath: string): { oldContent: string; newContent: string } {
  const fileDiff = diff.files.find((f) => f.path === filePath);
  if (!fileDiff) return { oldContent: '', newContent: '' };

  const oldLines: string[] = [];
  const newLines: string[] = [];

  for (const hunk of fileDiff.hunks) {
    for (const line of hunk.lines) {
      switch (line.kind) {
        case 'context':
          oldLines.push(line.content);
          newLines.push(line.content);
          break;
        case 'deletion':
          oldLines.push(line.content);
          break;
        case 'addition':
          newLines.push(line.content);
          break;
      }
    }
  }

  return {
    oldContent: oldLines.join('\n'),
    newContent: newLines.join('\n'),
  };
}

/** Get the file name from a full path. */
function fileName(path: string): string {
  const parts = path.split('/');
  return parts[parts.length - 1] || path;
}

/** Get directory portion of a path. */
function dirName(path: string): string {
  const idx = path.lastIndexOf('/');
  return idx > 0 ? path.slice(0, idx + 1) : '';
}

// ============================================================================
// HunkView Sub-component
// ============================================================================

interface HunkViewProps {
  hunk: DiffHunk;
  hunkIndex: number;
  filePath: string;
  isStaged: boolean;
  isOnlyHunk: boolean;
}

function HunkView({ hunk, hunkIndex, filePath, isStaged, isOnlyHunk }: HunkViewProps) {
  const { t } = useTranslation('git');
  const [isApplying, setIsApplying] = useState(false);
  const stageHunk = useGitStore((s) => s.stageHunk);

  const { oldContent, newContent } = hunkToContents(hunk);

  const additions = hunk.lines.filter((l) => l.kind === 'addition').length;
  const deletions = hunk.lines.filter((l) => l.kind === 'deletion').length;

  const handleStageHunk = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      setIsApplying(true);
      try {
        await stageHunk(filePath, hunkIndex, isStaged);
      } finally {
        setIsApplying(false);
      }
    },
    [stageHunk, filePath, hunkIndex, isStaged]
  );

  return (
    <div className="mb-2 last:mb-0">
      {/* Hunk header with stage/unstage button */}
      {!isOnlyHunk && (
        <div className="flex items-center justify-between px-2 py-1 bg-gray-100 dark:bg-gray-800/80 rounded-t border border-b-0 border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
            <span className="font-mono">{hunk.header.split('@@').slice(0, 2).join('@@') + ' @@'}</span>
            <span className="flex items-center gap-1">
              {additions > 0 && (
                <span className="text-green-600 dark:text-green-400">+{additions}</span>
              )}
              {deletions > 0 && (
                <span className="text-red-600 dark:text-red-400">-{deletions}</span>
              )}
            </span>
          </div>
          <button
            onClick={handleStageHunk}
            disabled={isApplying}
            className={clsx(
              'text-2xs px-2 py-0.5 rounded font-medium transition-colors',
              isApplying && 'opacity-50 cursor-not-allowed',
              isStaged
                ? 'text-orange-600 dark:text-orange-400 hover:bg-orange-50 dark:hover:bg-orange-900/20'
                : 'text-green-600 dark:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/20'
            )}
            title={isStaged ? t('fileEntry.unstageHunk') : t('fileEntry.stageHunk')}
          >
            {isApplying ? (
              <span className="flex items-center gap-1">
                <span className="animate-spin h-2.5 w-2.5 border border-current border-t-transparent rounded-full" />
              </span>
            ) : isStaged ? (
              t('fileEntry.unstageHunk')
            ) : (
              t('fileEntry.stageHunk')
            )}
          </button>
        </div>
      )}

      {/* Diff content */}
      <div className={clsx(!isOnlyHunk && 'border border-t-0 border-gray-200 dark:border-gray-700 rounded-b overflow-hidden')}>
        <EnhancedDiffViewer
          oldContent={oldContent}
          newContent={newContent}
          filePath={filePath}
          maxHeight={250}
        />
      </div>
    </div>
  );
}

// ============================================================================
// FileEntry Component
// ============================================================================

export function FileEntry({ file, isStaged, isUntracked }: FileEntryProps) {
  const { t } = useTranslation('git');
  const [expanded, setExpanded] = useState(false);
  const [diffData, setDiffData] = useState<DiffOutput | null>(null);
  const [loadingDiff, setLoadingDiff] = useState(false);

  const stageFiles = useGitStore((s) => s.stageFiles);
  const unstageFiles = useGitStore((s) => s.unstageFiles);
  const discardChanges = useGitStore((s) => s.discardChanges);

  const badge = getBadgeInfo(file.kind);
  const dir = dirName(file.path);
  const name = fileName(file.path);

  const handleToggleStage = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (isStaged) {
        await unstageFiles([file.path]);
      } else {
        await stageFiles([file.path]);
      }
    },
    [file.path, isStaged, stageFiles, unstageFiles]
  );

  const handleDiscard = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      await discardChanges([file.path]);
    },
    [file.path, discardChanges]
  );

  const handleToggleExpand = useCallback(async () => {
    const willExpand = !expanded;
    setExpanded(willExpand);

    if (willExpand && !diffData) {
      setLoadingDiff(true);
      try {
        const diff = await useGitStore.getState().getDiffForFile(file.path);
        setDiffData(diff);
      } finally {
        setLoadingDiff(false);
      }
    }
  }, [expanded, diffData, file.path]);

  // Extract hunks for hunk-level display
  const hunks = diffData?.files.find((f) => f.path === file.path)?.hunks ?? [];
  const hasMultipleHunks = hunks.length > 1;
  const contents = diffData ? diffOutputToContents(diffData, file.path) : null;

  return (
    <div className="border-b border-gray-100 dark:border-gray-800 last:border-b-0">
      {/* File row */}
      <div
        onClick={handleToggleExpand}
        className={clsx(
          'flex items-center gap-2 px-2 py-1.5 cursor-pointer transition-colors',
          'hover:bg-gray-50 dark:hover:bg-gray-800/60',
          expanded && 'bg-gray-50 dark:bg-gray-800/40'
        )}
      >
        {/* Expand chevron */}
        <svg
          className={clsx(
            'w-3 h-3 text-gray-400 dark:text-gray-500 transition-transform shrink-0',
            expanded && 'rotate-90'
          )}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>

        {/* Change type badge */}
        <span
          className={clsx(
            'text-2xs font-bold w-5 h-5 flex items-center justify-center rounded shrink-0',
            badge.className
          )}
        >
          {badge.letter}
        </span>

        {/* File path */}
        <span className="flex-1 min-w-0 text-xs truncate">
          {dir && (
            <span className="text-gray-400 dark:text-gray-500">{dir}</span>
          )}
          <span className="text-gray-800 dark:text-gray-200 font-medium">{name}</span>
        </span>

        {/* Hunk count indicator */}
        {hasMultipleHunks && expanded && (
          <span className="text-2xs text-gray-400 dark:text-gray-500 shrink-0">
            {t('fileEntry.hunks', { count: hunks.length })}
          </span>
        )}

        {/* Action buttons */}
        <div className="flex items-center gap-1 shrink-0">
          {/* Discard button (only for unstaged/untracked, not staged) */}
          {!isStaged && !isUntracked && (
            <button
              onClick={handleDiscard}
              className="p-1 rounded text-gray-400 hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
              title={t('fileEntry.discardChanges')}
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          )}

          {/* Stage/Unstage toggle */}
          <button
            onClick={handleToggleStage}
            className={clsx(
              'p-1 rounded transition-colors',
              isStaged
                ? 'text-green-600 dark:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/20'
                : 'text-gray-400 hover:text-green-600 dark:hover:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/20'
            )}
            title={isStaged ? t('fileEntry.unstageFile') : t('fileEntry.stageFile')}
          >
            {isStaged ? (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
              </svg>
            ) : (
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
            )}
          </button>
        </div>
      </div>

      {/* Expanded diff view */}
      {expanded && (
        <div className="px-2 pb-2">
          {loadingDiff && (
            <div className="flex items-center gap-2 py-3 justify-center text-xs text-gray-500 dark:text-gray-400">
              <div className="animate-spin h-3 w-3 border-2 border-gray-400 border-t-transparent rounded-full" />
              <span>{t('fileEntry.loadingDiff')}</span>
            </div>
          )}

          {/* Multiple hunks: show individual hunk views with stage buttons */}
          {!loadingDiff && hasMultipleHunks && hunks.map((hunk, idx) => (
            <HunkView
              key={`hunk-${idx}-${hunk.header}`}
              hunk={hunk}
              hunkIndex={idx}
              filePath={file.path}
              isStaged={isStaged}
              isOnlyHunk={false}
            />
          ))}

          {/* Single hunk or fallback: show unified diff with single stage button */}
          {!loadingDiff && !hasMultipleHunks && hunks.length === 1 && (
            <HunkView
              hunk={hunks[0]}
              hunkIndex={0}
              filePath={file.path}
              isStaged={isStaged}
              isOnlyHunk={true}
            />
          )}

          {/* No hunks but we have contents (fallback for new/deleted files) */}
          {!loadingDiff && hunks.length === 0 && contents && (
            <EnhancedDiffViewer
              oldContent={contents.oldContent}
              newContent={contents.newContent}
              filePath={file.path}
              maxHeight={300}
            />
          )}

          {!loadingDiff && !diffData && (
            <div className="py-3 text-center text-xs text-gray-500 dark:text-gray-400">
              {t('fileEntry.noDiffAvailable')}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default FileEntry;
