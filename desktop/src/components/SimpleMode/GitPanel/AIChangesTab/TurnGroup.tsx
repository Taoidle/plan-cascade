/**
 * TurnGroup Component
 *
 * Displays all file changes for a single conversation turn.
 * Collapsible header with turn number, change count, timestamp, and restore button.
 */

import { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import {
  ChevronRightIcon,
  ChevronDownIcon,
  ResetIcon,
} from '@radix-ui/react-icons';
import type { TurnChanges, RestoredFile } from '../../../../types/fileChanges';
import { useFileChangesStore } from '../../../../store/fileChanges';
import { FileChangeEntry } from './FileChangeEntry';
import { RestoreConfirmDialog } from './RestoreConfirmDialog';

interface TurnGroupProps {
  turn: TurnChanges;
  sessionId: string;
  projectRoot: string;
  onRestoreComplete?: (turnIndex: number, restored: RestoredFile[]) => void;
}

export function TurnGroup({ turn, sessionId, projectRoot, onRestoreComplete }: TurnGroupProps) {
  const { t } = useTranslation('git');
  const [collapsed, setCollapsed] = useState(false);
  const [showRestoreDialog, setShowRestoreDialog] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [restoreResult, setRestoreResult] = useState<RestoredFile[] | null>(null);
  const groupRef = useRef<HTMLDivElement>(null);

  const restoreToTurn = useFileChangesStore((s) => s.restoreToTurn);
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

  const handleRestoreClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setShowRestoreDialog(true);
    setRestoreResult(null);
  }, []);

  const handleConfirmRestore = useCallback(async () => {
    setRestoring(true);
    const result = await restoreToTurn(sessionId, projectRoot, turn.turn_index);
    setRestoring(false);
    if (result) {
      setRestoreResult(result);
      onRestoreComplete?.(turn.turn_index, result);
    }
  }, [restoreToTurn, sessionId, projectRoot, turn.turn_index, onRestoreComplete]);

  const handleCloseDialog = useCallback(() => {
    setShowRestoreDialog(false);
    setRestoreResult(null);
  }, []);

  const expectedFiles = useMemo(() => {
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
          {collapsed ? (
            <ChevronRightIcon className="w-3.5 h-3.5 text-gray-400 shrink-0" />
          ) : (
            <ChevronDownIcon className="w-3.5 h-3.5 text-gray-400 shrink-0" />
          )}

          <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('aiChanges.turn', { index: turn.turn_index })}
          </span>

          <span className="text-2xs text-gray-400 dark:text-gray-500">
            {t('aiChanges.files', { count: turn.changes.length })}
          </span>

          <span className="text-2xs text-gray-400 dark:text-gray-500 ml-auto mr-2">
            {timeStr}
          </span>

          {/* Restore button */}
          <span
            role="button"
            tabIndex={0}
            onClick={handleRestoreClick}
            onKeyDown={(e) => e.key === 'Enter' && handleRestoreClick(e as unknown as React.MouseEvent)}
            className={clsx(
              'flex items-center gap-1 px-1.5 py-0.5 rounded text-2xs font-medium',
              'text-orange-600 dark:text-orange-400',
              'opacity-0 group-hover:opacity-100 transition-opacity',
              'hover:bg-orange-100 dark:hover:bg-orange-900/30',
            )}
            title={t('aiChanges.restoreTitle', { index: turn.turn_index })}
          >
            <ResetIcon className="w-3 h-3" />
            {t('aiChanges.restore')}
          </span>
        </button>

        {/* File changes list */}
        {!collapsed && (
          <div className="pl-3">
            {turn.changes.map((change) => (
              <FileChangeEntry
                key={change.id}
                change={change}
                sessionId={sessionId}
                projectRoot={projectRoot}
              />
            ))}
          </div>
        )}
      </div>

      {/* Restore confirmation dialog */}
      {showRestoreDialog && (
        <RestoreConfirmDialog
          turnIndex={turn.turn_index}
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
