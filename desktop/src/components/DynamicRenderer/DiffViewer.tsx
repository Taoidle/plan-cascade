/**
 * DiffViewer Component
 *
 * Renders code diffs with syntax highlighting styling.
 * Used by DynamicRenderer for 'diff' component type.
 *
 * Story 002: DynamicRenderer frontend component
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import type { DiffData } from '../../types/richContent';

// ============================================================================
// Helpers
// ============================================================================

interface DiffLine {
  type: 'added' | 'removed' | 'context';
  content: string;
  lineNumberOld: number | null;
  lineNumberNew: number | null;
}

/**
 * Simple diff algorithm: compare lines and mark differences.
 * Uses a basic approach (not Myers diff) for simplicity.
 */
function computeDiffLines(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split('\n');
  const newLines = newText.split('\n');
  const lines: DiffLine[] = [];

  let oldIdx = 0;
  let newIdx = 0;

  // Simple LCS-inspired approach
  const oldSet = new Set(oldLines);
  const newSet = new Set(newLines);

  while (oldIdx < oldLines.length || newIdx < newLines.length) {
    if (oldIdx < oldLines.length && newIdx < newLines.length) {
      if (oldLines[oldIdx] === newLines[newIdx]) {
        // Context line
        lines.push({
          type: 'context',
          content: oldLines[oldIdx],
          lineNumberOld: oldIdx + 1,
          lineNumberNew: newIdx + 1,
        });
        oldIdx++;
        newIdx++;
      } else if (!newSet.has(oldLines[oldIdx])) {
        // Old line was removed
        lines.push({
          type: 'removed',
          content: oldLines[oldIdx],
          lineNumberOld: oldIdx + 1,
          lineNumberNew: null,
        });
        oldIdx++;
      } else if (!oldSet.has(newLines[newIdx])) {
        // New line was added
        lines.push({
          type: 'added',
          content: newLines[newIdx],
          lineNumberOld: null,
          lineNumberNew: newIdx + 1,
        });
        newIdx++;
      } else {
        // Both lines exist elsewhere; treat as remove + add
        lines.push({
          type: 'removed',
          content: oldLines[oldIdx],
          lineNumberOld: oldIdx + 1,
          lineNumberNew: null,
        });
        lines.push({
          type: 'added',
          content: newLines[newIdx],
          lineNumberOld: null,
          lineNumberNew: newIdx + 1,
        });
        oldIdx++;
        newIdx++;
      }
    } else if (oldIdx < oldLines.length) {
      lines.push({
        type: 'removed',
        content: oldLines[oldIdx],
        lineNumberOld: oldIdx + 1,
        lineNumberNew: null,
      });
      oldIdx++;
    } else {
      lines.push({
        type: 'added',
        content: newLines[newIdx],
        lineNumberOld: null,
        lineNumberNew: newIdx + 1,
      });
      newIdx++;
    }
  }

  return lines;
}

// ============================================================================
// Component
// ============================================================================

interface DiffViewerProps {
  data: DiffData;
}

export function DiffViewer({ data }: DiffViewerProps) {
  const diffLines = useMemo(() => computeDiffLines(data.old, data.new), [data.old, data.new]);

  const addedCount = diffLines.filter((l) => l.type === 'added').length;
  const removedCount = diffLines.filter((l) => l.type === 'removed').length;

  return (
    <div className="space-y-2" data-testid="diff-viewer">
      {/* Header */}
      <div className="flex items-center gap-3 text-xs">
        {data.file && <span className="font-mono text-gray-600 dark:text-gray-400 truncate">{data.file}</span>}
        {data.language && (
          <span className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
            {data.language}
          </span>
        )}
        <div className="flex items-center gap-2 ml-auto">
          <span className="text-green-600 dark:text-green-400">+{addedCount}</span>
          <span className="text-red-600 dark:text-red-400">-{removedCount}</span>
        </div>
      </div>

      {/* Diff content */}
      <div
        className={clsx(
          'overflow-x-auto rounded-lg border',
          'border-gray-200 dark:border-gray-700',
          'bg-gray-50 dark:bg-gray-900',
          'font-mono text-xs',
        )}
      >
        <table className="w-full border-collapse">
          <tbody>
            {diffLines.map((line, idx) => (
              <tr
                key={idx}
                className={clsx(
                  line.type === 'added' && 'bg-green-50 dark:bg-green-900/20',
                  line.type === 'removed' && 'bg-red-50 dark:bg-red-900/20',
                )}
              >
                {/* Old line number */}
                <td className="px-2 py-0.5 text-right text-gray-400 dark:text-gray-600 select-none w-10 border-r border-gray-200 dark:border-gray-700">
                  {line.lineNumberOld ?? ''}
                </td>
                {/* New line number */}
                <td className="px-2 py-0.5 text-right text-gray-400 dark:text-gray-600 select-none w-10 border-r border-gray-200 dark:border-gray-700">
                  {line.lineNumberNew ?? ''}
                </td>
                {/* Indicator */}
                <td
                  className={clsx(
                    'px-1 py-0.5 w-5 text-center select-none',
                    line.type === 'added' && 'text-green-600 dark:text-green-400',
                    line.type === 'removed' && 'text-red-600 dark:text-red-400',
                    line.type === 'context' && 'text-gray-400',
                  )}
                >
                  {line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' '}
                </td>
                {/* Content */}
                <td className="px-2 py-0.5 whitespace-pre text-gray-800 dark:text-gray-200">{line.content}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default DiffViewer;
