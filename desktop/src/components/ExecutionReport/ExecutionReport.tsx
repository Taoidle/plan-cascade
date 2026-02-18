/**
 * ExecutionReport Component
 *
 * Main component that integrates all report sub-components:
 * SummaryCard, QualityRadarChart, TimelineWaterfall, and AgentPerformanceTable.
 * Includes export buttons for JSON and Markdown.
 *
 * Story 004: Execution Report visualization components
 */

import { useCallback } from 'react';
import { clsx } from 'clsx';
import { DownloadIcon, FileTextIcon, CodeIcon } from '@radix-ui/react-icons';
import { useExecutionReportStore } from '../../store/executionReport';
import { SummaryCard } from './SummaryCard';
import { QualityRadarChart } from './QualityRadarChart';
import { TimelineWaterfall } from './TimelineWaterfall';
import { AgentPerformanceTable } from './AgentPerformanceTable';

// ============================================================================
// Component
// ============================================================================

export function ExecutionReport() {
  const { report, exportAsJson, exportAsMarkdown } = useExecutionReportStore();

  const handleExportJson = useCallback(() => {
    const json = exportAsJson();
    if (!json) return;

    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `execution-report-${report?.sessionId ?? 'unknown'}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [exportAsJson, report]);

  const handleExportMarkdown = useCallback(() => {
    const md = exportAsMarkdown();
    if (!md) return;

    const blob = new Blob([md], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `execution-report-${report?.sessionId ?? 'unknown'}.md`;
    a.click();
    URL.revokeObjectURL(url);
  }, [exportAsMarkdown, report]);

  if (!report) {
    return null;
  }

  return (
    <div className="space-y-4" data-testid="execution-report">
      {/* Header with export buttons */}
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-800 dark:text-gray-200">
          Execution Report
        </h3>
        <div className="flex items-center gap-2">
          <button
            onClick={handleExportJson}
            className={clsx(
              'flex items-center gap-1.5 px-2 py-1 rounded text-xs',
              'border border-gray-300 dark:border-gray-600',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'transition-colors'
            )}
            title="Export as JSON"
          >
            <CodeIcon className="w-3.5 h-3.5" />
            JSON
          </button>
          <button
            onClick={handleExportMarkdown}
            className={clsx(
              'flex items-center gap-1.5 px-2 py-1 rounded text-xs',
              'border border-gray-300 dark:border-gray-600',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'transition-colors'
            )}
            title="Export as Markdown"
          >
            <FileTextIcon className="w-3.5 h-3.5" />
            Markdown
          </button>
        </div>
      </div>

      {/* Summary Card */}
      <SummaryCard summary={report.summary} />

      {/* Quality Radar Chart */}
      {report.radarDimensions.length > 0 && (
        <div
          className={clsx(
            'p-4 rounded-lg',
            'border border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800'
          )}
        >
          <QualityRadarChart dimensions={report.radarDimensions} />
        </div>
      )}

      {/* Timeline Waterfall */}
      {report.timeline.length > 0 && (
        <div
          className={clsx(
            'p-4 rounded-lg',
            'border border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800'
          )}
        >
          <TimelineWaterfall
            entries={report.timeline}
            totalDurationMs={report.summary.totalTimeMs}
          />
        </div>
      )}

      {/* Agent Performance */}
      {report.agentPerformance.length > 0 && (
        <div
          className={clsx(
            'p-4 rounded-lg',
            'border border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800'
          )}
        >
          <AgentPerformanceTable agents={report.agentPerformance} />
        </div>
      )}
    </div>
  );
}

export default ExecutionReport;
