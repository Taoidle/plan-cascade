/**
 * TurnGroup Component
 *
 * Displays all file changes for a single conversation turn.
 * Collapsible header with turn number, change count, timestamp, and restore button.
 */

import { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { ChevronRightIcon, ResetIcon } from '@radix-ui/react-icons';
import type { TurnChanges, RestoredFile } from '../../../../types/fileChanges';
import { useFileChangesStore } from '../../../../store/fileChanges';
import { FileChangeEntry } from './FileChangeEntry';
import { RestoreConfirmDialog } from './RestoreConfirmDialog';
import { Collapsible } from '../../Collapsible';

interface TurnGroupProps {
  turn: TurnChanges;
  sessionId: string;
  projectRoot: string;
  onRestoreComplete?: (turnIndex: number, restored: RestoredFile[]) => void;
  onUndoComplete?: (restored: RestoredFile[]) => void;
}

export function TurnGroup({ turn, sessionId, projectRoot, onRestoreComplete, onUndoComplete }: TurnGroupProps) {
  const { t } = useTranslation('git');
  const [collapsed, setCollapsed] = useState(false);
  const [showRestoreDialog, setShowRestoreDialog] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewFiles, setPreviewFiles] = useState<{ path: string; willDelete: boolean }[] | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [undoing, setUndoing] = useState(false);
  const [restoreResult, setRestoreResult] = useState<RestoredFile[] | null>(null);
  const [restoreOperationId, setRestoreOperationId] = useState<string | null>(null);
  const groupRef = useRef<HTMLDivElement>(null);

  const previewRestoreToTurn = useFileChangesStore((s) => s.previewRestoreToTurn);
  const restoreToTurn = useFileChangesStore((s) => s.restoreToTurn);
  const undoRestore = useFileChangesStore((s) => s.undoRestore);
  const selectedTurnIndex = useFileChangesStore((s) => s.selectedTurnIndex);

  // Auto-expand and scroll into view when selected from chat card
  useEffect(() => {
    if (selectedTurnIndex === turn.turn_index) {
      setCollapsed(false);
      // Scroll into view with a small delay to allow DOM update
      requestAnimationFrame(() => {
        groupRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
    }
  }, [selectedTurnIndex, turn.turn_index]);

  const handleToggle = useCallback(() => {
    setCollapsed((prev) => !prev);
  }, []);

  const fallbackExpectedFiles = useMemo(() => {
    // Deduplicate by file path — only the earliest change per file matters
    const seen = new Map<string, boolean>();
    for (const change of turn.changes) {
      if (!seen.has(change.file_path)) {
        seen.set(change.file_path, change.before_hash === null);
      }
    }
    return Array.from(seen.entries()).map(([path, willDelete]) => ({
      path,
      willDelete,
    }));
  }, [turn.changes]);

  const handleRestoreClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setShowRestoreDialog(true);
    setRestoreResult(null);
    setRestoreOperationId(null);
  }, []);

  useEffect(() => {
    if (!showRestoreDialog) return;
    setPreviewLoading(true);
    previewRestoreToTurn(sessionId, projectRoot, turn.turn_index)
      .then((items) => {
        if (items && items.length > 0) {
          setPreviewFiles(
            items.map((item) => ({
              path: item.path,
              willDelete: item.action === 'delete',
            })),
          );
          return;
        }
        setPreviewFiles(fallbackExpectedFiles);
      })
      .finally(() => setPreviewLoading(false));
  }, [showRestoreDialog, previewRestoreToTurn, sessionId, projectRoot, turn.turn_index, fallbackExpectedFiles]);

  const handleConfirmRestore = useCallback(async () => {
    setRestoring(true);
    const result = await restoreToTurn(sessionId, projectRoot, turn.turn_index, true);
    setRestoring(false);
    if (result) {
      setRestoreResult(result.restored);
      setRestoreOperationId(result.operation_id);
      onRestoreComplete?.(turn.turn_index, result.restored);
    }
  }, [restoreToTurn, sessionId, projectRoot, turn.turn_index, onRestoreComplete]);

  const handleUndoRestore = useCallback(async () => {
    if (!restoreOperationId) return;
    setUndoing(true);
    const undone = await undoRestore(sessionId, projectRoot, restoreOperationId);
    setUndoing(false);
    if (undone) {
      setRestoreResult(undone);
      setRestoreOperationId(null);
      onUndoComplete?.(undone);
    }
  }, [undoRestore, sessionId, projectRoot, restoreOperationId, onUndoComplete]);

  const handleCloseDialog = useCallback(() => {
    setShowRestoreDialog(false);
    setPreviewLoading(false);
    setPreviewFiles(null);
    setRestoreResult(null);
    setRestoreOperationId(null);
    setUndoing(false);
  }, []);

  const timeStr = useMemo(() => {
    const d = new Date(turn.timestamp);
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  }, [turn.timestamp]);

  return (
    <>
      <div ref={groupRef} className="border-b border-gray-100 dark:border-gray-800 last:border-b-0">
        {/* Turn header */}
        <button
          onClick={handleToggle}
          className="flex items-center gap-2 w-full px-3 py-2 text-left hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors group"
        >
          <ChevronRightIcon
            className={clsx(
              'w-3.5 h-3.5 text-gray-400 shrink-0 transition-transform duration-200',
              !collapsed && 'rotate-90',
            )}
          />

          <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('aiChanges.turn', { index: turn.turn_index })}
          </span>

          <span className="text-2xs text-gray-400 dark:text-gray-500">
            {t('aiChanges.files', { count: turn.changes.length })}
          </span>

          <span className="text-2xs text-gray-400 dark:text-gray-500 ml-auto mr-2">{timeStr}</span>

          {/* Restore button */}
          <span
            role="button"
            tabIndex={0}
            onClick={handleRestoreClick}
            onKeyDown={(e) => e.key === 'Enter' && handleRestoreClick(e as unknown as React.MouseEvent)}
            className={clsx(
              'flex items-center gap-1 px-1.5 py-0.5 rounded text-2xs font-medium',
              'text-orange-600 dark:text-orange-400',
              'opacity-80 hover:opacity-100 transition-opacity',
              'hover:bg-orange-100 dark:hover:bg-orange-900/30',
            )}
            title={t('aiChanges.restoreTitle', { index: turn.turn_index })}
          >
            <ResetIcon className="w-3 h-3" />
            {t('aiChanges.restore')}
          </span>
        </button>

        {/* File changes list */}
        <Collapsible open={!collapsed}>
          <div className="pl-3">
            {turn.changes.map((change) => (
              <FileChangeEntry key={change.id} change={change} sessionId={sessionId} projectRoot={projectRoot} />
            ))}
          </div>
        </Collapsible>
      </div>

      {/* Restore confirmation dialog */}
      {showRestoreDialog && (
        <RestoreConfirmDialog
          turnIndex={turn.turn_index}
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
