/**
 * ProgressChart Component
 *
 * Renders a story progress visualization (horizontal bar chart).
 * Used by DynamicRenderer for 'chart' component type.
 *
 * Story 002: DynamicRenderer frontend component
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import type { ChartData, ChartDataItem } from '../../types/richContent';

// ============================================================================
// Helpers
// ============================================================================

const statusColors: Record<string, string> = {
  success: 'bg-green-500',
  error: 'bg-red-500',
  warning: 'bg-amber-500',
  info: 'bg-blue-500',
  neutral: 'bg-gray-400',
};

function getBarColor(item: ChartDataItem): string {
  if (item.color) return item.color;
  if (item.status) return statusColors[item.status] ?? statusColors.neutral;
  return statusColors.info;
}

// ============================================================================
// Component
// ============================================================================

interface ProgressChartProps {
  data: ChartData;
}

export function ProgressChart({ data }: ProgressChartProps) {
  const total = useMemo(() => {
    return data.total ?? data.items.reduce((sum, item) => sum + item.value, 0);
  }, [data]);

  return (
    <div className="space-y-3" data-testid="progress-chart">
      {data.title && (
        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {data.title}
        </h4>
      )}

      {/* Stacked bar */}
      {total > 0 && (
        <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden flex">
          {data.items.map((item, idx) => {
            const percent = (item.value / total) * 100;
            if (percent <= 0) return null;

            const barColorClass = getBarColor(item);
            const isCustomColor = item.color && !item.status;

            return (
              <div
                key={idx}
                className={clsx(
                  'h-full transition-all duration-300',
                  !isCustomColor && barColorClass
                )}
                style={{
                  width: `${percent}%`,
                  ...(isCustomColor ? { backgroundColor: item.color } : {}),
                }}
                title={`${item.label}: ${item.value} (${percent.toFixed(1)}%)`}
              />
            );
          })}
        </div>
      )}

      {/* Legend */}
      <div className="flex flex-wrap gap-3">
        {data.items.map((item, idx) => {
          const barColorClass = getBarColor(item);
          const isCustomColor = item.color && !item.status;
          const percent = total > 0 ? ((item.value / total) * 100).toFixed(1) : '0';

          return (
            <div key={idx} className="flex items-center gap-1.5 text-xs">
              <div
                className={clsx('w-3 h-3 rounded-sm', !isCustomColor && barColorClass)}
                style={isCustomColor ? { backgroundColor: item.color } : undefined}
              />
              <span className="text-gray-600 dark:text-gray-400">
                {item.label}:
              </span>
              <span className="font-medium text-gray-800 dark:text-gray-200">
                {item.value}
              </span>
              <span className="text-gray-400 dark:text-gray-500">
                ({percent}%)
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default ProgressChart;
