/**
 * GitPanel Component
 *
 * Main container for the git source control panel. Replaces the former DiffPanel
 * with a tabbed layout: Changes (default), History, Branches.
 * Bottom area reserved for ToolChangesBar.
 */

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useGitStore } from '../../../store/git';
import { useGitStatus } from '../../../hooks/useGitStatus';
import { TabBar } from './TabBar';
import { ChangesTab } from './ChangesTab';
import { HistoryTab } from './HistoryTab';
import { BranchesTab } from './BranchesTab';
import { ToolChangesBar } from './ToolChangesBar';
import type { StreamLine } from '../../../store/execution';

// Re-export sub-components for consumers
export { BranchesTab } from './BranchesTab';
export { MergeBar } from './MergeBar';
export { ConflictResolver } from './ConflictResolver';

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
// GitPanel Component
// ============================================================================

export function GitPanel({ streamingOutput, workspacePath }: GitPanelProps) {
  const { t } = useTranslation('git');
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
      { id: 'changes' as const, label: t('tabs.changes'), count: stagedCount },
      { id: 'history' as const, label: t('tabs.history'), count: commitLog.length },
      { id: 'branches' as const, label: t('tabs.branches'), count: branches.length },
    ];
  }, [status, commitLog.length, branches.length, t]);

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
        {selectedTab === 'history' && <HistoryTab />}
        {selectedTab === 'branches' && <BranchesTab />}
      </div>

      {/* Bottom persistent bar for tool changes */}
      <ToolChangesBar streamingOutput={streamingOutput} />
    </div>
  );
}

export default GitPanel;
