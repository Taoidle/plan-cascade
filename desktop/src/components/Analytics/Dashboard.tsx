/**
 * Analytics Dashboard Component
 *
 * Main dashboard view for usage analytics with overview cards,
 * period selector, charts, and data visualization.
 */

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useAnalyticsStore } from '../../store/analytics';
import { OverviewCards } from './OverviewCards';
import { PeriodSelector } from './PeriodSelector';
import { CostChart } from './CostChart';
import { TokenBreakdown } from './TokenBreakdown';
import { ExportDialog } from './ExportDialog';
import { UsageTable } from './UsageTable';
import AnalyticsSkeleton from './AnalyticsSkeleton';

export function Dashboard() {
  const { t } = useTranslation('analytics');
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [activeTab, setActiveTab] = useState<'overview' | 'details'>('overview');

  const {
    summary,
    isLoading,
    error,
    initialize,
    fetchDashboardSummary,
    fetchPricing,
    clearError,
  } = useAnalyticsStore();

  // Initialize and fetch data on mount
  useEffect(() => {
    const init = async () => {
      await initialize();
      await fetchDashboardSummary();
      await fetchPricing();
    };
    init();
  }, [initialize, fetchDashboardSummary, fetchPricing]);

  // Handle period change
  const handlePeriodChange = async () => {
    await fetchDashboardSummary();
  };

  // Handle retry
  const handleRetry = () => {
    clearError();
    fetchDashboardSummary();
  };

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Header */}
      <div
        className={clsx(
          'flex items-center justify-between px-6 py-4',
          'border-b border-gray-200 dark:border-gray-800',
          'bg-white dark:bg-gray-900',
          'shrink-0'
        )}
      >
        <div>
          <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
            {t('title', 'Usage Analytics')}
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            {t('subtitle', 'Track your API usage and costs')}
          </p>
        </div>

        <div className="flex items-center gap-4">
          <PeriodSelector onChange={handlePeriodChange} />

          <button
            onClick={() => setShowExportDialog(true)}
            className={clsx(
              'px-4 py-2 rounded-lg',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
              'transition-colors'
            )}
          >
            {t('export', 'Export')}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div
        className={clsx(
          'flex border-b border-gray-200 dark:border-gray-800',
          'bg-white dark:bg-gray-900',
          'px-6',
          'shrink-0'
        )}
      >
        <button
          onClick={() => setActiveTab('overview')}
          className={clsx(
            'px-4 py-3 text-sm font-medium',
            'border-b-2 transition-colors',
            activeTab === 'overview'
              ? 'border-primary-500 text-primary-600 dark:text-primary-400'
              : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
          )}
        >
          {t('tabs.overview', 'Overview')}
        </button>
        <button
          onClick={() => setActiveTab('details')}
          className={clsx(
            'px-4 py-3 text-sm font-medium',
            'border-b-2 transition-colors',
            activeTab === 'details'
              ? 'border-primary-500 text-primary-600 dark:text-primary-400'
              : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
          )}
        >
          {t('tabs.details', 'Detailed Records')}
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6 bg-gray-50 dark:bg-gray-950">
        {/* Error State */}
        {error && (
          <div
            className={clsx(
              'mb-6 p-4 rounded-lg',
              'bg-red-50 dark:bg-red-900/20',
              'border border-red-200 dark:border-red-800',
              'flex items-center justify-between'
            )}
          >
            <div className="flex items-center gap-3">
              <svg
                className="w-5 h-5 text-red-500"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <span className="text-red-700 dark:text-red-300">{error}</span>
            </div>
            <button
              onClick={handleRetry}
              className={clsx(
                'px-3 py-1 rounded',
                'bg-red-100 dark:bg-red-800',
                'text-red-700 dark:text-red-200',
                'hover:bg-red-200 dark:hover:bg-red-700'
              )}
            >
              {t('retry', 'Retry')}
            </button>
          </div>
        )}

        {/* Loading State */}
        {isLoading && !summary && <AnalyticsSkeleton />}

        {/* Overview Tab */}
        {activeTab === 'overview' && summary && (
          <div className="space-y-6">
            {/* Overview Cards */}
            <OverviewCards summary={summary} isLoading={isLoading} />

            {/* Charts Grid */}
            <div className="grid grid-cols-1 lg:grid-cols-2 3xl:grid-cols-2 gap-6">
              {/* Cost Over Time Chart */}
              <div
                className={clsx(
                  'bg-white dark:bg-gray-900 rounded-xl',
                  'border border-gray-200 dark:border-gray-800',
                  'p-6'
                )}
              >
                <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
                  {t('charts.costOverTime', 'Cost Over Time')}
                </h3>
                <CostChart data={summary.time_series} />
              </div>

              {/* Token Breakdown */}
              <div
                className={clsx(
                  'bg-white dark:bg-gray-900 rounded-xl',
                  'border border-gray-200 dark:border-gray-800',
                  'p-6'
                )}
              >
                <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
                  {t('charts.tokenBreakdown', 'Usage by Model')}
                </h3>
                <TokenBreakdown byModel={summary.by_model} byProject={summary.by_project} />
              </div>
            </div>

            {/* Top Models Table */}
            {summary.by_model.length > 0 && (
              <div
                className={clsx(
                  'bg-white dark:bg-gray-900 rounded-xl',
                  'border border-gray-200 dark:border-gray-800',
                  'p-6'
                )}
              >
                <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
                  {t('tables.topModels', 'Top Models by Cost')}
                </h3>
                <ModelTable models={summary.by_model} />
              </div>
            )}
          </div>
        )}

        {/* Details Tab */}
        {activeTab === 'details' && <UsageTable />}
      </div>

      {/* Export Dialog */}
      <ExportDialog open={showExportDialog} onOpenChange={setShowExportDialog} />
    </div>
  );
}

// Model usage table component
interface ModelTableProps {
  models: Array<{
    model_name: string;
    provider: string;
    stats: {
      total_input_tokens: number;
      total_output_tokens: number;
      total_cost_microdollars: number;
      request_count: number;
    };
  }>;
}

function ModelTable({ models }: ModelTableProps) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-gray-500 dark:text-gray-400 border-b border-gray-200 dark:border-gray-800">
            <th className="pb-3 font-medium">Model</th>
            <th className="pb-3 font-medium">Provider</th>
            <th className="pb-3 font-medium text-right">Requests</th>
            <th className="pb-3 font-medium text-right">Input Tokens</th>
            <th className="pb-3 font-medium text-right">Output Tokens</th>
            <th className="pb-3 font-medium text-right">Cost</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
          {models.map((model) => (
            <tr
              key={`${model.provider}-${model.model_name}`}
              className="text-gray-900 dark:text-white"
            >
              <td className="py-3 font-medium">{model.model_name}</td>
              <td className="py-3 capitalize">{model.provider}</td>
              <td className="py-3 text-right">{model.stats.request_count.toLocaleString()}</td>
              <td className="py-3 text-right">
                {(model.stats.total_input_tokens / 1000).toFixed(1)}K
              </td>
              <td className="py-3 text-right">
                {(model.stats.total_output_tokens / 1000).toFixed(1)}K
              </td>
              <td className="py-3 text-right font-medium">
                ${(model.stats.total_cost_microdollars / 1_000_000).toFixed(4)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default Dashboard;
