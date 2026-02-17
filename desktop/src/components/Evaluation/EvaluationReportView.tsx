/**
 * EvaluationReportView Component
 *
 * Displays evaluation reports with:
 * - Score table (model x case)
 * - Overall model ranking
 * - Cost and duration comparisons
 * - Individual test result details
 */

import { clsx } from 'clsx';
import { useEvaluationStore } from '../../store/evaluation';

export function EvaluationReportView() {
  const {
    selectedRunId,
    reports,
    runs,
    loading,
    selectRun,
    setActiveTab,
  } = useEvaluationStore();

  if (loading.reports) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 dark:text-gray-400">
        Loading reports...
      </div>
    );
  }

  if (!selectedRunId || reports.length === 0) {
    return (
      <div className="p-6 max-w-4xl mx-auto">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          Evaluation Reports
        </h2>

        {runs.filter((r) => r.status === 'completed').length === 0 ? (
          <div className="text-center py-12 text-gray-500 dark:text-gray-400">
            <p className="text-sm">No completed runs yet</p>
            <p className="text-xs mt-1">
              Create and run an evaluation from the Runs tab
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
              Select a completed run to view reports:
            </p>
            {runs
              .filter((r) => r.status === 'completed')
              .map((run) => (
                <button
                  key={run.id}
                  onClick={() => selectRun(run.id)}
                  className={clsx(
                    'w-full text-left p-3 rounded-lg border transition-colors',
                    'border-gray-200 dark:border-gray-700',
                    'hover:border-primary-300 dark:hover:border-primary-700',
                    'bg-white dark:bg-gray-800'
                  )}
                >
                  <div className="text-sm font-medium text-gray-900 dark:text-white">
                    Run {run.id.slice(0, 8)}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                    {run.model_count} models | {run.case_count} cases | {new Date(run.created_at).toLocaleDateString()}
                  </div>
                </button>
              ))}
          </div>
        )}
      </div>
    );
  }

  // Sort reports by score descending
  const sortedReports = [...reports].sort((a, b) => b.overall_score - a.overall_score);

  // Get all unique case IDs
  const allCaseIds = new Set<string>();
  for (const report of reports) {
    for (const result of report.results) {
      allCaseIds.add(result.case_id);
    }
  }
  const caseIds = Array.from(allCaseIds);

  return (
    <div className="p-6 max-w-6xl mx-auto space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            Evaluation Reports
          </h2>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
            Run {selectedRunId.slice(0, 8)} | {reports.length} model{reports.length !== 1 ? 's' : ''} compared
          </p>
        </div>
        <button
          onClick={() => {
            setActiveTab('runs');
          }}
          className="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
        >
          Back to Runs
        </button>
      </div>

      {/* Model Ranking */}
      <div>
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">
          Model Ranking
        </h3>
        <div className="space-y-2">
          {sortedReports.map((report, rank) => (
            <div
              key={`${report.provider}-${report.model}`}
              className={clsx(
                'flex items-center gap-4 p-3 rounded-lg border',
                rank === 0
                  ? 'border-yellow-300 dark:border-yellow-700 bg-yellow-50 dark:bg-yellow-900/10'
                  : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800'
              )}
            >
              <span
                className={clsx(
                  'w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold shrink-0',
                  rank === 0
                    ? 'bg-yellow-200 dark:bg-yellow-800 text-yellow-800 dark:text-yellow-200'
                    : 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400'
                )}
              >
                {rank + 1}
              </span>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium text-gray-900 dark:text-white truncate">
                  {report.provider}/{report.model}
                </div>
                <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                  {report.results.filter((r) => r.passed).length}/{report.results.length} passed
                </div>
              </div>
              <div className="text-right shrink-0">
                <div className={clsx(
                  'text-lg font-bold',
                  report.overall_score >= 0.8
                    ? 'text-green-600 dark:text-green-400'
                    : report.overall_score >= 0.5
                    ? 'text-yellow-600 dark:text-yellow-400'
                    : 'text-red-600 dark:text-red-400'
                )}>
                  {(report.overall_score * 100).toFixed(1)}%
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Score Table (Model x Case) */}
      {caseIds.length > 0 && (
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">
            Score Table
          </h3>
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-gray-200 dark:border-gray-700">
                  <th className="text-left py-2 px-2 text-gray-500 dark:text-gray-400 font-medium">
                    Model
                  </th>
                  {caseIds.map((caseId) => (
                    <th
                      key={caseId}
                      className="text-center py-2 px-2 text-gray-500 dark:text-gray-400 font-medium"
                    >
                      {caseId.slice(0, 12)}
                    </th>
                  ))}
                  <th className="text-center py-2 px-2 text-gray-500 dark:text-gray-400 font-medium">
                    Overall
                  </th>
                </tr>
              </thead>
              <tbody>
                {sortedReports.map((report) => (
                  <tr
                    key={`${report.provider}-${report.model}`}
                    className="border-b border-gray-100 dark:border-gray-800"
                  >
                    <td className="py-2 px-2 font-mono text-gray-900 dark:text-white">
                      {report.model}
                    </td>
                    {caseIds.map((caseId) => {
                      const result = report.results.find((r) => r.case_id === caseId);
                      return (
                        <td key={caseId} className="text-center py-2 px-2">
                          {result ? (
                            <span
                              className={clsx(
                                'inline-block px-1.5 py-0.5 rounded',
                                result.passed
                                  ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                                  : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
                              )}
                            >
                              {(result.score * 100).toFixed(0)}%
                            </span>
                          ) : (
                            <span className="text-gray-400">-</span>
                          )}
                        </td>
                      );
                    })}
                    <td className="text-center py-2 px-2 font-bold">
                      <span
                        className={clsx(
                          report.overall_score >= 0.8
                            ? 'text-green-600 dark:text-green-400'
                            : report.overall_score >= 0.5
                            ? 'text-yellow-600 dark:text-yellow-400'
                            : 'text-red-600 dark:text-red-400'
                        )}
                      >
                        {(report.overall_score * 100).toFixed(1)}%
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Cost & Duration Comparison */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Duration */}
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">
            Duration Comparison
          </h3>
          <div className="space-y-2">
            {sortedReports.map((report) => {
              const maxDuration = Math.max(...reports.map((r) => r.duration_ms));
              const pct = maxDuration > 0 ? (report.duration_ms / maxDuration) * 100 : 0;

              return (
                <div key={`dur-${report.provider}-${report.model}`}>
                  <div className="flex justify-between text-xs mb-0.5">
                    <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
                      {report.model}
                    </span>
                    <span className="text-gray-500 dark:text-gray-400 shrink-0 ml-2">
                      {(report.duration_ms / 1000).toFixed(1)}s
                    </span>
                  </div>
                  <div className="h-2 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-blue-500 rounded-full"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Cost */}
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">
            Cost Comparison
          </h3>
          <div className="space-y-2">
            {sortedReports.map((report) => {
              const maxCost = Math.max(...reports.map((r) => r.estimated_cost));
              const pct = maxCost > 0 ? (report.estimated_cost / maxCost) * 100 : 0;

              return (
                <div key={`cost-${report.provider}-${report.model}`}>
                  <div className="flex justify-between text-xs mb-0.5">
                    <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
                      {report.model}
                    </span>
                    <span className="text-gray-500 dark:text-gray-400 shrink-0 ml-2">
                      ${report.estimated_cost.toFixed(4)} | {report.total_tokens.toLocaleString()} tokens
                    </span>
                  </div>
                  <div className="h-2 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-amber-500 rounded-full"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
