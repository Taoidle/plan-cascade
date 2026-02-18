/**
 * StagingArea Component
 *
 * Three collapsible sections: Staged Changes, Unstaged Changes, Untracked Files.
 * Each section header shows a file count and Stage All / Unstage All button.
 * Uses FileEntry component for each file.
 *
 * Feature-002, Story-004
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useGitStore } from '../../../../store/git';
import { FileEntry } from './FileEntry';

// ============================================================================
// Section Header
// ============================================================================

interface SectionProps {
  title: string;
  count: number;
  defaultOpen?: boolean;
  actionLabel?: string;
  onAction?: () => void;
  children: React.ReactNode;
}

function CollapsibleSection({
  title,
  count,
  defaultOpen = true,
  actionLabel,
  onAction,
  children,
}: SectionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="border-b border-gray-200 dark:border-gray-700 last:border-b-0">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-gray-50 dark:bg-gray-800/50">
        <button
          onClick={() => setOpen((v) => !v)}
          className="flex items-center gap-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-gray-100 transition-colors"
        >
          <svg
            className={clsx('w-3 h-3 transition-transform', open && 'rotate-90')}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
          <span>{title}</span>
          <span className="text-2xs text-gray-500 dark:text-gray-400 ml-1">({count})</span>
        </button>

        {actionLabel && onAction && count > 0 && (
          <button
            onClick={onAction}
            className="text-2xs px-2 py-0.5 rounded text-primary-600 dark:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-colors"
          >
            {actionLabel}
          </button>
        )}
      </div>

      {/* Content */}
      {open && count > 0 && <div>{children}</div>}
      {open && count === 0 && (
        <div className="px-3 py-3 text-center text-2xs text-gray-400 dark:text-gray-500">
          No files
        </div>
      )}
    </div>
  );
}

// ============================================================================
// StagingArea Component
// ============================================================================

export function StagingArea() {
  const status = useGitStore((s) => s.status);
  const stageFiles = useGitStore((s) => s.stageFiles);
  const unstageFiles = useGitStore((s) => s.unstageFiles);
  const stageAll = useGitStore((s) => s.stageAll);

  const staged = status?.staged ?? [];
  const unstaged = status?.unstaged ?? [];
  const untracked = status?.untracked ?? [];

  const handleUnstageAll = useCallback(() => {
    if (staged.length > 0) {
      unstageFiles(staged.map((f) => f.path));
    }
  }, [staged, unstageFiles]);

  const handleStageAllUnstaged = useCallback(() => {
    if (unstaged.length > 0) {
      stageFiles(unstaged.map((f) => f.path));
    }
  }, [unstaged, stageFiles]);

  const handleStageAllUntracked = useCallback(() => {
    if (untracked.length > 0) {
      stageFiles(untracked.map((f) => f.path));
    }
  }, [untracked, stageFiles]);

  if (!status) {
    return (
      <div className="flex items-center justify-center py-8 text-sm text-gray-500 dark:text-gray-400">
        <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full mr-2" />
        Loading status...
      </div>
    );
  }

  const totalChanges = staged.length + unstaged.length + untracked.length;
  if (totalChanges === 0 && status.conflicted.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-8 text-sm text-gray-500 dark:text-gray-400">
        <svg className="w-8 h-8 mb-2 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 13l4 4L19 7" />
        </svg>
        <p>Working tree clean</p>
        <p className="text-2xs text-gray-400 dark:text-gray-500 mt-1">
          {status.branch && `on ${status.branch}`}
          {status.upstream && ` tracking ${status.upstream}`}
        </p>
      </div>
    );
  }

  return (
    <div>
      {/* Staged Changes */}
      <CollapsibleSection
        title="Staged Changes"
        count={staged.length}
        defaultOpen={true}
        actionLabel="Unstage All"
        onAction={handleUnstageAll}
      >
        {staged.map((file) => (
          <FileEntry key={`staged-${file.path}`} file={file} isStaged={true} />
        ))}
      </CollapsibleSection>

      {/* Unstaged Changes */}
      <CollapsibleSection
        title="Changes"
        count={unstaged.length}
        defaultOpen={true}
        actionLabel="Stage All"
        onAction={handleStageAllUnstaged}
      >
        {unstaged.map((file) => (
          <FileEntry key={`unstaged-${file.path}`} file={file} isStaged={false} />
        ))}
      </CollapsibleSection>

      {/* Untracked Files */}
      <CollapsibleSection
        title="Untracked"
        count={untracked.length}
        defaultOpen={true}
        actionLabel="Stage All"
        onAction={handleStageAllUntracked}
      >
        {untracked.map((file) => (
          <FileEntry key={`untracked-${file.path}`} file={file} isStaged={false} isUntracked={true} />
        ))}
      </CollapsibleSection>
    </div>
  );
}

export default StagingArea;
