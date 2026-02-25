/**
 * AIChangesTab Component
 *
 * Lists LLM file modifications grouped by conversation turn.
 * Each turn group shows file changes with expandable diffs and a restore button.
 * Integrates with the file change tracking backend via the fileChanges store.
 */

import { useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { useFileChangesStore } from '../../../../store/fileChanges';
import { useGitStore } from '../../../../store/git';
import { TurnGroup } from './TurnGroup';
import type { RestoredFile } from '../../../../types/fileChanges';

interface AIChangesTabProps {
  sessionId: string | null;
  projectRoot: string | null;
}

export function AIChangesTab({ sessionId, projectRoot }: AIChangesTabProps) {
  const { t } = useTranslation('git');
  const turnChanges = useFileChangesStore((s) => s.turnChanges);
  const loading = useFileChangesStore((s) => s.loading);
  const error = useFileChangesStore((s) => s.error);
  const fetchChanges = useFileChangesStore((s) => s.fetchChanges);
  const truncateFromTurn = useFileChangesStore((s) => s.truncateFromTurn);
  const refreshGitStatus = useGitStore((s) => s.refreshStatus);

  // Fetch changes on mount and when session changes
  useEffect(() => {
    if (sessionId && projectRoot) {
      fetchChanges(sessionId, projectRoot);
    }
  }, [sessionId, projectRoot, fetchChanges]);

  // Listen for file-change-recorded events to auto-refresh
  useEffect(() => {
    const unlisten = listen('file-change-recorded', () => {
      if (sessionId && projectRoot) {
        fetchChanges(sessionId, projectRoot);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [sessionId, projectRoot, fetchChanges]);

  const handleRestoreComplete = useCallback(
    async (turnIndex: number, _restored: RestoredFile[]) => {
      if (!sessionId || !projectRoot) return;
      // Truncate change records from the restored turn onward
      await truncateFromTurn(sessionId, projectRoot, turnIndex);
      // Refresh both AI changes and git status
      await fetchChanges(sessionId, projectRoot);
      refreshGitStatus();
    },
    [sessionId, projectRoot, truncateFromTurn, fetchChanges, refreshGitStatus],
  );

  // Empty state
  if (!sessionId || !projectRoot) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-gray-400 dark:text-gray-500 p-4">
        {t('aiChanges.noSession')}
      </div>
    );
  }

  if (loading && turnChanges.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-gray-400 dark:text-gray-500 p-4">
        {t('aiChanges.loading')}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-red-500 dark:text-red-400 p-4">{error}</div>
    );
  }

  if (turnChanges.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-gray-400 dark:text-gray-500 p-4">
        {t('aiChanges.noChanges')}
      </div>
    );
  }

  return (
    <div className="divide-y divide-gray-100 dark:divide-gray-800">
      {turnChanges.map((turn) => (
        <TurnGroup
          key={turn.turn_index}
          turn={turn}
          sessionId={sessionId}
          projectRoot={projectRoot}
          onRestoreComplete={handleRestoreComplete}
        />
      ))}
    </div>
  );
}

export default AIChangesTab;
