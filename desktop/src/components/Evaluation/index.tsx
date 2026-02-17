/**
 * EvaluationDashboard Component
 *
 * Main container for the Evaluation Framework in Expert Mode.
 * Provides tabs for:
 * - Setup: Define evaluators with criteria
 * - Runs: Create and manage evaluation runs
 * - Reports: View evaluation results and model comparisons
 */

import { useEffect } from 'react';
import { clsx } from 'clsx';
import { useEvaluationStore } from '../../store/evaluation';
import type { EvaluationTab } from '../../store/evaluation';
import { EvaluatorSetup } from './EvaluatorSetup';
import { EvaluationRunList } from './EvaluationRunList';
import { EvaluationReportView } from './EvaluationReportView';

export function EvaluationDashboard() {
  const { activeTab, setActiveTab, fetchEvaluators, fetchRuns, error } = useEvaluationStore();

  useEffect(() => {
    fetchEvaluators();
    fetchRuns();
  }, [fetchEvaluators, fetchRuns]);

  const tabs: { value: EvaluationTab; label: string }[] = [
    { value: 'setup', label: 'Evaluators' },
    { value: 'runs', label: 'Runs' },
    { value: 'reports', label: 'Reports' },
  ];

  return (
    <div className="h-full flex flex-col">
      {/* Tab bar */}
      <div
        className={clsx(
          'flex items-center gap-1 px-6 py-2',
          'border-b border-gray-200 dark:border-gray-700'
        )}
      >
        {tabs.map((tab) => (
          <button
            key={tab.value}
            onClick={() => setActiveTab(tab.value)}
            className={clsx(
              'px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
              activeTab === tab.value
                ? 'text-primary-600 dark:text-primary-400 bg-gray-100 dark:bg-gray-800'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Error display */}
      {error && (
        <div className="mx-6 mt-2 p-2 rounded-lg bg-red-50 dark:bg-red-900/20 text-xs text-red-600 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Tab content */}
      <div className="flex-1 overflow-auto">
        {activeTab === 'setup' && <EvaluatorSetup />}
        {activeTab === 'runs' && <EvaluationRunList />}
        {activeTab === 'reports' && <EvaluationReportView />}
      </div>
    </div>
  );
}

export default EvaluationDashboard;
