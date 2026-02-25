/**
 * TimelineWaterfall Component
 *
 * Renders a horizontal waterfall chart showing batch execution
 * with agent assignments, durations, and gate results.
 *
 * Story 004: Execution Report visualization components
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import { CheckCircledIcon, CrossCircledIcon } from '@radix-ui/react-icons';
import type { TimelineEntry } from '../../store/executionReport';

// ============================================================================
// Helpers
// ============================================================================

const agentColors: Record<string, string> = {
  'claude-sonnet': 'bg-blue-400',
  'claude-haiku': 'bg-cyan-400',
  'claude-opus': 'bg-purple-400',
  'gpt-4': 'bg-green-400',
  'gpt-3.5': 'bg-emerald-400',
  deepseek: 'bg-amber-400',
  unknown: 'bg-gray-400',
};

function getAgentColor(agent: string): string {
  return agentColors[agent] ?? agentColors['unknown'];
}

// ============================================================================
// Component
// ============================================================================

interface TimelineWaterfallProps {
  entries: TimelineEntry[];
  totalDurationMs: number;
}

export function TimelineWaterfall({ entries, totalDurationMs }: TimelineWaterfallProps) {
  // Group by batch
  const batchGroups = useMemo(() => {
    const groups = new Map<number, TimelineEntry[]>();
    for (const entry of entries) {
      const batch = groups.get(entry.batchIndex) ?? [];
      batch.push(entry);
      groups.set(entry.batchIndex, batch);
    }
    return Array.from(groups.entries()).sort(([a], [b]) => a - b);
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="text-xs text-gray-400 dark:text-gray-500 italic" data-testid="timeline-waterfall">
        No timeline data
      </div>
    );
  }

  const maxDuration = totalDurationMs > 0 ? totalDurationMs : 1;

  return (
    <div className="space-y-3" data-testid="timeline-waterfall">
      <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">Execution Timeline</h4>

      <div className="space-y-1">
        {batchGroups.map(([batchIndex, batchEntries]) => (
          <div key={batchIndex} className="space-y-0.5">
            {/* Batch header */}
            <div className="text-[10px] text-gray-400 dark:text-gray-500 font-medium uppercase tracking-wider">
              Batch {batchIndex}
            </div>

            {/* Entries in batch */}
            {batchEntries.map((entry) => {
              const left = (entry.startOffsetMs / maxDuration) * 100;
              const width = Math.max((entry.durationMs / maxDuration) * 100, 2);

              return (
                <div key={entry.storyId} className="flex items-center gap-2 h-6">
                  {/* Story label */}
                  <div className="w-28 truncate text-xs text-gray-600 dark:text-gray-400 text-right flex-shrink-0">
                    {entry.storyTitle}
                  </div>

                  {/* Waterfall bar */}
                  <div className="flex-1 relative h-5 bg-gray-100 dark:bg-gray-800 rounded overflow-hidden">
                    <div
                      className={clsx(
                        'absolute h-full rounded transition-all',
                        entry.status === 'completed' ? getAgentColor(entry.agent) : 'bg-red-400',
                        'opacity-70 hover:opacity-100',
                      )}
                      style={{
                        left: `${left}%`,
                        width: `${width}%`,
                      }}
                      title={`${entry.storyTitle} | ${entry.agent} | ${(entry.durationMs / 1000).toFixed(1)}s`}
                    />
                  </div>

                  {/* Status icon */}
                  <div className="w-5 flex-shrink-0 flex items-center justify-center">
                    {entry.status === 'completed' ? (
                      <CheckCircledIcon className="w-3.5 h-3.5 text-green-500" />
                    ) : entry.status === 'failed' ? (
                      <CrossCircledIcon className="w-3.5 h-3.5 text-red-500" />
                    ) : null}
                  </div>

                  {/* Duration */}
                  <div className="w-12 text-xs text-gray-500 dark:text-gray-400 text-right flex-shrink-0">
                    {(entry.durationMs / 1000).toFixed(1)}s
                  </div>
                </div>
              );
            })}
          </div>
        ))}
      </div>

      {/* Agent color legend */}
      <div className="flex flex-wrap gap-3 pt-2 border-t border-gray-200 dark:border-gray-700">
        {Array.from(new Set(entries.map((e) => e.agent))).map((agent) => (
          <div key={agent} className="flex items-center gap-1.5 text-xs">
            <div className={clsx('w-3 h-3 rounded-sm', getAgentColor(agent))} />
            <span className="text-gray-600 dark:text-gray-400">{agent}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default TimelineWaterfall;
