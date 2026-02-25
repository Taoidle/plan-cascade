/**
 * ImportanceBar Component
 *
 * Displays a visual bar representing memory importance (0.0 - 1.0).
 * Color gradient from gray (low) through yellow (medium) to red (high).
 */

import { clsx } from 'clsx';

interface ImportanceBarProps {
  /** Importance value from 0.0 to 1.0 */
  value: number;
  className?: string;
  /** Show numeric label */
  showLabel?: boolean;
}

function getBarColor(value: number): string {
  if (value >= 0.8) return 'bg-red-500 dark:bg-red-400';
  if (value >= 0.6) return 'bg-amber-500 dark:bg-amber-400';
  if (value >= 0.4) return 'bg-yellow-500 dark:bg-yellow-400';
  return 'bg-gray-400 dark:bg-gray-500';
}

export function ImportanceBar({ value, className, showLabel = false }: ImportanceBarProps) {
  const clamped = Math.max(0, Math.min(1, value));
  const widthPercent = Math.round(clamped * 100);

  return (
    <div data-testid="importance-bar" className={clsx('flex items-center gap-1.5', className)}>
      <div className="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden min-w-[3rem]">
        <div
          className={clsx('h-full rounded-full transition-all', getBarColor(clamped))}
          style={{ width: `${widthPercent}%` }}
          role="progressbar"
          aria-valuenow={widthPercent}
          aria-valuemin={0}
          aria-valuemax={100}
        />
      </div>
      {showLabel && (
        <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0 w-7 text-right">
          {(clamped * 100).toFixed(0)}%
        </span>
      )}
    </div>
  );
}

export default ImportanceBar;
