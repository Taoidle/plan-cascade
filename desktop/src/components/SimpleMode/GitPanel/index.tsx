/**
 * GitPanel Component
 *
 * Main container for the git source control panel. Replaces the former DiffPanel
 * with a tabbed layout: Changes (default), History, Branches.
 * Bottom area reserved for ToolChangesBar.
 *
 * Feature-002: Changes Tab with Staging Workflow & Commit UI
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import { useGitStore } from '../../../store/git';
import { useGitStatus } from '../../../hooks/useGitStatus';
import { TabBar } from './TabBar';
import { ChangesTab } from './ChangesTab';
import { ToolChangesBar } from './ToolChangesBar';
import type { StreamLine } from '../../../store/execution';

// ============================================================================
// Types
// ============================================================================

interface GitPanelProps {
  /** Full streaming output from the current session. */
  streamingOutput: StreamLine[];
  /** Current workspace directory path, or null if none selected. */
  workspacePath: string | null;
}

// ============================================================================
// Placeholder components for History and Branches tabs (future features)
// ============================================================================

function HistoryTabPlaceholder() {
  return (
    <div className="flex-1 flex items-center justify-center p-6 text-sm text-gray-500 dark:text-gray-400">
      <div className="text-center">
        <svg className="w-8 h-8 mx-auto mb-2 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p>Commit history will be available in a future update.</p>
      </div>
    </div>
  );
}

function BranchesTabPlaceholder() {
  return (
    <div className="flex-1 flex items-center justify-center p-6 text-sm text-gray-500 dark:text-gray-400">
      <div className="text-center">
        <svg className="w-8 h-8 mx-auto mb-2 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <p>Branch management will be available in a future update.</p>
      </div>
    </div>
  );
}

// ============================================================================
// GitPanel Component
// ============================================================================

export function GitPanel({ streamingOutput, workspacePath }: GitPanelProps) {
  const selectedTab = useGitStore((s) => s.selectedTab);
  const status = useGitStore((s) => s.status);
  const commitLog = useGitStore((s) => s.commitLog);
  const branches = useGitStore((s) => s.branches);

  // Initialize git status polling / event subscription
  useGitStatus();

  // Build tab definitions with counts
  const tabs = useMemo(() => {
    const stagedCount = (status?.staged.length ?? 0) +
      (status?.unstaged.length ?? 0) +
      (status?.untracked.length ?? 0);

    return [
      { id: 'changes' as const, label: 'Changes', count: stagedCount },
      { id: 'history' as const, label: 'History', count: commitLog.length },
      { id: 'branches' as const, label: 'Branches', count: branches.length },
    ];
  }, [status, commitLog.length, branches.length]);

  return (
    <div
      className={clsx(
        'min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700',
        'bg-white dark:bg-gray-900 overflow-hidden'
      )}
    >
      {/* Tab Bar */}
      <div className="shrink-0">
        <TabBar tabs={tabs} />
      </div>

      {/* Tab Content */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {selectedTab === 'changes' && <ChangesTab />}
        {selectedTab === 'history' && <HistoryTabPlaceholder />}
        {selectedTab === 'branches' && <BranchesTabPlaceholder />}
      </div>

      {/* Bottom persistent bar for tool changes */}
      <ToolChangesBar streamingOutput={streamingOutput} />
    </div>
  );
}

export default GitPanel;
