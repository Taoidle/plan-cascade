/**
 * AgentPerformanceTable Component
 *
 * Compares agent performance: success rate, speed, quality score.
 *
 * Story 004: Execution Report visualization components
 */

import { clsx } from 'clsx';
import type { AgentPerformance } from '../../store/executionReport';

// ============================================================================
// Component
// ============================================================================

interface AgentPerformanceTableProps {
  agents: AgentPerformance[];
}

export function AgentPerformanceTable({ agents }: AgentPerformanceTableProps) {
  if (agents.length === 0) {
    return (
      <div className="text-xs text-gray-400 dark:text-gray-500 italic" data-testid="agent-performance-table">
        No agent data
      </div>
    );
  }

  return (
    <div className="space-y-3" data-testid="agent-performance-table">
      <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">Agent Performance</h4>

      <div className="overflow-x-auto">
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr className="border-b border-gray-200 dark:border-gray-700">
              <th className="px-3 py-2 text-left font-medium text-gray-500 dark:text-gray-400">Agent</th>
              <th className="px-3 py-2 text-right font-medium text-gray-500 dark:text-gray-400">Stories</th>
              <th className="px-3 py-2 text-right font-medium text-gray-500 dark:text-gray-400">Completed</th>
              <th className="px-3 py-2 text-right font-medium text-gray-500 dark:text-gray-400">Success Rate</th>
              <th className="px-3 py-2 text-right font-medium text-gray-500 dark:text-gray-400">Avg Duration</th>
              <th className="px-3 py-2 text-right font-medium text-gray-500 dark:text-gray-400">Quality</th>
            </tr>
          </thead>
          <tbody>
            {agents.map((agent) => (
              <tr
                key={agent.agentName}
                className={clsx(
                  'border-b border-gray-100 dark:border-gray-800',
                  'hover:bg-gray-50 dark:hover:bg-gray-800/50',
                  'transition-colors',
                )}
              >
                <td className="px-3 py-2 font-medium text-gray-700 dark:text-gray-300">{agent.agentName}</td>
                <td className="px-3 py-2 text-right text-gray-600 dark:text-gray-400">{agent.storiesAssigned}</td>
                <td className="px-3 py-2 text-right text-green-600 dark:text-green-400">{agent.storiesCompleted}</td>
                <td className="px-3 py-2 text-right">
                  <span
                    className={clsx(
                      'font-medium',
                      agent.successRate >= 80
                        ? 'text-green-600 dark:text-green-400'
                        : agent.successRate >= 50
                          ? 'text-amber-600 dark:text-amber-400'
                          : 'text-red-600 dark:text-red-400',
                    )}
                  >
                    {agent.successRate}%
                  </span>
                </td>
                <td className="px-3 py-2 text-right text-gray-600 dark:text-gray-400">
                  {(agent.averageDurationMs / 1000).toFixed(1)}s
                </td>
                <td className="px-3 py-2 text-right text-gray-600 dark:text-gray-400">
                  {agent.averageQualityScore !== null ? agent.averageQualityScore.toFixed(1) : '-'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default AgentPerformanceTable;
