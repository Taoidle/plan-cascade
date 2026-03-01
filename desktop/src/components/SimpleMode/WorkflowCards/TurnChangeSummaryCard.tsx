/**
 * TurnChangeSummaryCard Component
 *
 * Inline chat card summarizing all file changes in a conversation turn.
 * Only injected when 2+ files were changed in the same turn.
 */

import { useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useFileChangesStore } from '../../../store/fileChanges';
import { useSettingsStore } from '../../../store/settings';
import { useGitStore } from '../../../store/git';
import { RestoreConfirmDialog } from '../GitPanel/AIChangesTab/RestoreConfirmDialog';
import type { TurnChangeSummaryCardData } from '../../../types/workflowCard';
import type { RestoredFile } from '../../../types/fileChanges';
import { requestOpenAIChanges } from '../../../lib/simpleModeNavigation';

export function TurnChangeSummaryCard({ data }: { data: TurnChangeSummaryCardData }) {
  const { t } = useTranslation('simpleMode');
  const [showRestoreDialog, setShowRestoreDialog] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewFiles, setPreviewFiles] = useState<{ path: string; willDelete: boolean }[] | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [undoing, setUndoing] = useState(false);
  const [restoreResult, setRestoreResult] = useState<RestoredFile[] | null>(null);
  const [restoreOperationId, setRestoreOperationId] = useState<string | null>(null);

  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const refreshGitStatus = useGitStore((s) => s.refreshStatus);
  const fetchChanges = useFileChangesStore((s) => s.fetchChanges);
  const previewRestoreToTurn = useFileChangesStore((s) => s.previewRestoreToTurn);
  const restoreToTurn = useFileChangesStore((s) => s.restoreToTurn);
  const undoRestore = useFileChangesStore((s) => s.undoRestore);

  const handleViewAll = useCallback(() => {
    requestOpenAIChanges({ turnIndex: data.turnIndex });
  }, [data.turnIndex]);

  const fallbackExpectedFiles = useMemo(
    () =>
      data.files.map((f) => ({
        path: f.filePath,
        willDelete: f.changeType === 'new_file',
      })),
    [data.files],
  );

  const handleRevertAll = useCallback(async () => {
    setShowRestoreDialog(true);
    setRestoreResult(null);
    setRestoreOperationId(null);
    if (!workspacePath) {
      setPreviewFiles(fallbackExpectedFiles);
      return;
    }
    setPreviewLoading(true);
    const preview = await previewRestoreToTurn(data.sessionId, workspacePath, data.turnIndex);
    setPreviewLoading(false);
    if (preview && preview.length > 0) {
      setPreviewFiles(
        preview.map((item) => ({
          path: item.path,
          willDelete: item.action === 'delete',
        })),
      );
      return;
    }
    setPreviewFiles(fallbackExpectedFiles);
  }, [workspacePath, fallbackExpectedFiles, previewRestoreToTurn, data.sessionId, data.turnIndex]);

  const handleConfirmRestore = useCallback(async () => {
    if (!workspacePath) return;
    setRestoring(true);
    const result = await restoreToTurn(data.sessionId, workspacePath, data.turnIndex, true);
    setRestoring(false);
    if (result) {
      setRestoreResult(result.restored);
      setRestoreOperationId(result.operation_id);
      await fetchChanges(data.sessionId, workspacePath);
      refreshGitStatus();
    }
  }, [restoreToTurn, fetchChanges, refreshGitStatus, data.sessionId, data.turnIndex, workspacePath]);

  const handleUndoRestore = useCallback(async () => {
    if (!workspacePath || !restoreOperationId) return;
    setUndoing(true);
    const undone = await undoRestore(data.sessionId, workspacePath, restoreOperationId);
    setUndoing(false);
    if (undone) {
      setRestoreResult(undone);
      setRestoreOperationId(null);
      await fetchChanges(data.sessionId, workspacePath);
      refreshGitStatus();
    }
  }, [undoRestore, fetchChanges, refreshGitStatus, workspacePath, restoreOperationId, data.sessionId]);

  const handleCloseDialog = useCallback(() => {
    setShowRestoreDialog(false);
    setPreviewLoading(false);
    setPreviewFiles(null);
    setRestoreResult(null);
    setRestoreOperationId(null);
    setUndoing(false);
  }, []);

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
            {t('workflow.turnSummary.files', { count: data.totalFiles })}{' '}
            <span className="font-mono">
              <span className="text-green-600 dark:text-green-400">+{data.totalLinesAdded}</span>{' '}
              <span className="text-red-600 dark:text-red-400">-{data.totalLinesRemoved}</span>
            </span>
          </span>
        </div>

        {/* File list */}
        <div className="border-t border-gray-200/50 dark:border-gray-700/50">
          {data.files.map((file, i) => (
            <div key={i} className="flex items-center gap-2 px-3 py-1 text-2xs">
              <span className="text-gray-400 dark:text-gray-500">●</span>
              <span className="font-mono text-gray-700 dark:text-gray-300 truncate flex-1">{file.filePath}</span>
              <span
                className={clsx(
                  'shrink-0 rounded px-1 py-0.5 font-medium',
                  file.changeType === 'new_file'
                    ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                    : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
                )}
              >
                {file.changeType === 'new_file' ? t('workflow.fileChange.newFile') : t('workflow.fileChange.modified')}
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
          <button onClick={handleViewAll} className="text-2xs text-gray-600 dark:text-gray-400 hover:underline">
            {t('workflow.turnSummary.viewAll')}
          </button>
          <button onClick={handleRevertAll} className="text-2xs text-gray-600 dark:text-gray-400 hover:underline">
            {t('workflow.turnSummary.revertAll')}
          </button>
        </div>
      </div>

      {showRestoreDialog && (
        <RestoreConfirmDialog
          turnIndex={data.turnIndex}
          expectedFiles={previewFiles ?? fallbackExpectedFiles}
          previewLoading={previewLoading}
          onConfirm={handleConfirmRestore}
          onCancel={handleCloseDialog}
          restoring={restoring}
          result={restoreResult}
          onUndo={handleUndoRestore}
          canUndo={Boolean(restoreOperationId)}
          undoing={undoing}
        />
      )}
    </>
  );
}
