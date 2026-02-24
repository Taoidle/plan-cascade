/**
 * TurnChangeSummaryCard Component
 *
 * Inline chat card summarizing all file changes in a conversation turn.
 * Only injected when 2+ files were changed in the same turn.
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useFileChangesStore } from '../../../store/fileChanges';
import { useGitStore } from '../../../store/git';
import { RestoreConfirmDialog } from '../GitPanel/AIChangesTab/RestoreConfirmDialog';
import type { TurnChangeSummaryCardData } from '../../../types/workflowCard';
import type { RestoredFile } from '../../../types/fileChanges';

export function TurnChangeSummaryCard({ data }: { data: TurnChangeSummaryCardData }) {
  const { t } = useTranslation('simpleMode');
  const [showRestoreDialog, setShowRestoreDialog] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [restoreResult, setRestoreResult] = useState<RestoredFile[] | null>(null);

  const restoreToTurn = useFileChangesStore((s) => s.restoreToTurn);

  const handleViewAll = useCallback(() => {
    useGitStore.getState().setSelectedTab('ai-changes');
    useFileChangesStore.getState().selectTurn(data.turnIndex);
    useGitStore.getState().setDiffPanelVisible(true);
  }, [data.turnIndex]);

  const handleRevertAll = useCallback(() => {
    setShowRestoreDialog(true);
    setRestoreResult(null);
  }, []);

  const handleConfirmRestore = useCallback(async () => {
    setRestoring(true);
    const result = await restoreToTurn(data.sessionId, '', data.turnIndex);
    setRestoring(false);
    if (result) {
      setRestoreResult(result);
    }
  }, [restoreToTurn, data.sessionId, data.turnIndex]);

  const handleCloseDialog = useCallback(() => {
    setShowRestoreDialog(false);
    setRestoreResult(null);
  }, []);

  const expectedFiles = data.files.map((f) => ({
    path: f.filePath,
    willDelete: f.changeType === 'new_file',
  }));

  return (
    <>
      <div
        className={clsx(
          'rounded-lg border overflow-hidden',
          'border-gray-200 dark:border-gray-700',
          'bg-gray-50/50 dark:bg-gray-800/30',
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('workflow.turnSummary.title')}
          </span>
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            {t('workflow.turnSummary.files', { count: data.totalFiles })}
            {' '}
            <span className="font-mono">
              <span className="text-green-600 dark:text-green-400">+{data.totalLinesAdded}</span>
              {' '}
              <span className="text-red-600 dark:text-red-400">-{data.totalLinesRemoved}</span>
            </span>
          </span>
        </div>

        {/* File list */}
        <div className="border-t border-gray-200/50 dark:border-gray-700/50">
          {data.files.map((file, i) => (
            <div
              key={i}
              className="flex items-center gap-2 px-3 py-1 text-2xs"
            >
              <span className="text-gray-400 dark:text-gray-500">●</span>
              <span className="font-mono text-gray-700 dark:text-gray-300 truncate flex-1">
                {file.filePath}
              </span>
              <span
                className={clsx(
                  'shrink-0 rounded px-1 py-0.5 font-medium',
                  file.changeType === 'new_file'
                    ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                    : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
                )}
              >
                {file.changeType === 'new_file'
                  ? t('workflow.fileChange.newFile')
                  : t('workflow.fileChange.modified')}
              </span>
              <span className="shrink-0 font-mono text-gray-500 dark:text-gray-400">
                <span className="text-green-600 dark:text-green-400">+{file.linesAdded}</span>
                {file.linesRemoved > 0 && (
                  <>
                    {' '}
                    <span className="text-red-600 dark:text-red-400">-{file.linesRemoved}</span>
                  </>
                )}
              </span>
            </div>
          ))}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-3 px-3 py-1.5 border-t border-gray-200/50 dark:border-gray-700/50">
          <button
            onClick={handleViewAll}
            className="text-2xs text-gray-600 dark:text-gray-400 hover:underline"
          >
            {t('workflow.turnSummary.viewAll')}
          </button>
          <button
            onClick={handleRevertAll}
            className="text-2xs text-gray-600 dark:text-gray-400 hover:underline"
          >
            {t('workflow.turnSummary.revertAll')}
          </button>
        </div>
      </div>

      {showRestoreDialog && (
        <RestoreConfirmDialog
          turnIndex={data.turnIndex}
          expectedFiles={expectedFiles}
          onConfirm={handleConfirmRestore}
          onCancel={handleCloseDialog}
          restoring={restoring}
          result={restoreResult}
        />
      )}
    </>
  );
}
