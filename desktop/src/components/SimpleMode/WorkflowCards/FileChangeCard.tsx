/**
 * FileChangeCard Component
 *
 * Inline chat card showing a file change preview with diff, stats, and actions.
 * Rendered when the LLM's Write/Edit tool modifies a file.
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useFileChangesStore } from '../../../store/fileChanges';
import { useGitStore } from '../../../store/git';
import { DiffViewer } from '../../shared/DiffViewer';
import type { FileChangeCardData } from '../../../types/workflowCard';

export function FileChangeCard({ data }: { data: FileChangeCardData }) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);
  const [fullDiff, setFullDiff] = useState<string | null>(null);
  const [loadingDiff, setLoadingDiff] = useState(false);
  const [showRevertConfirm, setShowRevertConfirm] = useState(false);
  const [reverting, setReverting] = useState(false);
  const [reverted, setReverted] = useState(false);

  const fetchDiff = useFileChangesStore((s) => s.fetchDiff);
  const restoreSingleFile = useFileChangesStore((s) => s.restoreSingleFile);

  const fileName = data.filePath.split('/').pop() || data.filePath;
  const dirPath = data.filePath.includes('/') ? data.filePath.substring(0, data.filePath.lastIndexOf('/')) : '';

  const handleExpandDiff = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (fullDiff !== null) return;
    setLoadingDiff(true);
    const diff = await fetchDiff(
      data.sessionId,
      '', // projectRoot not needed — prefilled cache should have it
      data.changeId,
      data.beforeHash,
      data.afterHash,
    );
    setFullDiff(diff);
    setLoadingDiff(false);
  }, [expanded, fullDiff, fetchDiff, data]);

  const handleViewInChanges = useCallback(() => {
    useGitStore.getState().setSelectedTab('ai-changes');
    useFileChangesStore.getState().selectTurn(data.turnIndex);
    useGitStore.getState().setDiffPanelVisible(true);
  }, [data.turnIndex]);

  const handleRevert = useCallback(async () => {
    if (!showRevertConfirm) {
      setShowRevertConfirm(true);
      return;
    }
    if (!data.beforeHash) return; // Can't revert new file without before hash
    setReverting(true);
    const ok = await restoreSingleFile(data.sessionId, '', data.filePath, data.beforeHash);
    setReverting(false);
    if (ok) {
      setReverted(true);
      setShowRevertConfirm(false);
    }
  }, [showRevertConfirm, data, restoreSingleFile]);

  return (
    <div
      className={clsx(
        'rounded-lg border overflow-hidden',
        'border-amber-200 dark:border-amber-800/60',
        'bg-amber-50/50 dark:bg-amber-900/10',
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2">
        {/* Tool badge */}
        <span
          className={clsx(
            'inline-block rounded px-1.5 py-0.5 text-2xs font-medium shrink-0',
            data.toolName === 'Write'
              ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
              : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
          )}
        >
          {data.toolName}
        </span>

        {/* File path */}
        <span className="text-xs font-mono text-gray-700 dark:text-gray-300 truncate flex-1">
          {dirPath && <span className="text-gray-400 dark:text-gray-500">{dirPath}/</span>}
          {fileName}
        </span>

        {/* Change type badge */}
        <span
          className={clsx(
            'inline-block rounded px-1.5 py-0.5 text-2xs font-medium shrink-0',
            data.changeType === 'new_file'
              ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
              : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300',
          )}
        >
          {data.changeType === 'new_file' ? t('workflow.fileChange.newFile') : t('workflow.fileChange.modified')}
        </span>
      </div>

      {/* Diff preview */}
      {data.diffPreview && !expanded && (
        <div className="px-3 pb-1">
          <DiffViewer diffContent={data.diffPreview} maxLines={8} showTruncation />
        </div>
      )}

      {/* Expanded full diff */}
      {expanded && (
        <div className="px-3 pb-1">
          {loadingDiff ? (
            <p className="text-2xs text-gray-400 py-2">Loading diff...</p>
          ) : fullDiff ? (
            <DiffViewer diffContent={fullDiff} />
          ) : data.diffPreview ? (
            <DiffViewer diffContent={data.diffPreview} />
          ) : (
            <p className="text-2xs text-gray-400 py-2">No diff available</p>
          )}
        </div>
      )}

      {/* Footer: stats + actions */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-t border-amber-200/50 dark:border-amber-800/30">
        {/* Stats */}
        <span className="text-2xs font-mono text-gray-500 dark:text-gray-400">
          <span className="text-green-600 dark:text-green-400">+{data.linesAdded}</span>{' '}
          <span className="text-red-600 dark:text-red-400">-{data.linesRemoved}</span>
        </span>

        <div className="flex-1" />

        {/* Actions */}
        <button onClick={handleExpandDiff} className="text-2xs text-amber-700 dark:text-amber-400 hover:underline">
          {expanded ? t('workflow.fileChange.collapseDiff') : t('workflow.fileChange.expandDiff')}
        </button>

        <span className="text-gray-300 dark:text-gray-600">|</span>

        <button onClick={handleViewInChanges} className="text-2xs text-amber-700 dark:text-amber-400 hover:underline">
          {t('workflow.fileChange.viewInChanges')}
        </button>

        {data.beforeHash && !reverted && (
          <>
            <span className="text-gray-300 dark:text-gray-600">|</span>
            <button
              onClick={handleRevert}
              disabled={reverting}
              className={clsx(
                'text-2xs hover:underline',
                showRevertConfirm ? 'text-red-600 dark:text-red-400 font-medium' : 'text-amber-700 dark:text-amber-400',
                reverting && 'opacity-50',
              )}
            >
              {reverting
                ? '...'
                : showRevertConfirm
                  ? t('workflow.fileChange.revertConfirm')
                  : t('workflow.fileChange.revert')}
            </button>
          </>
        )}

        {reverted && <span className="text-2xs text-green-600 dark:text-green-400 font-medium">Reverted</span>}
      </div>
    </div>
  );
}
