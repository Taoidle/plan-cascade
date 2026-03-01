/**
 * Usage Table Component
 *
 * Displays detailed usage records in a paginated table.
 */

import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import {
  useAnalyticsStore,
  formatCost,
  formatTokens,
  type CostStatus,
  type DashboardFilterV2,
} from '../../store/analytics';

const SAVED_VIEWS_KEY = 'analytics_v2_saved_views';

interface SavedView {
  name: string;
  filter: DashboardFilterV2;
}

export function UsageTable() {
  const { t } = useTranslation('analytics');
  const [page, setPage] = useState(0);
  const pageSize = 20;

  const { records, totalRecords, recordsLoading, filter, fetchRecords, setFilter } = useAnalyticsStore();

  const [draft, setDraft] = useState<DashboardFilterV2>({
    provider: filter.provider,
    model_name: filter.model_name,
    project_id: filter.project_id,
    session_id: filter.session_id,
    cost_status: filter.cost_status,
  });
  const [viewName, setViewName] = useState('');
  const [savedViews, setSavedViews] = useState<SavedView[]>(() => {
    try {
      const raw = localStorage.getItem(SAVED_VIEWS_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  });

  useEffect(() => {
    setDraft((prev) => ({
      ...prev,
      provider: filter.provider,
      model_name: filter.model_name,
      project_id: filter.project_id,
      session_id: filter.session_id,
      cost_status: filter.cost_status,
    }));
  }, [filter.provider, filter.model_name, filter.project_id, filter.session_id, filter.cost_status]);

  useEffect(() => {
    fetchRecords(pageSize, page * pageSize);
  }, [fetchRecords, page, filter, pageSize]);

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const applyFilter = () => {
    setPage(0);
    setFilter({
      ...filter,
      provider: draft.provider?.trim() || undefined,
      model_name: draft.model_name?.trim() || undefined,
      project_id: draft.project_id?.trim() || undefined,
      session_id: draft.session_id?.trim() || undefined,
      cost_status: draft.cost_status,
    });
  };

  const clearAdvancedFilter = () => {
    const next = {
      ...filter,
      provider: undefined,
      model_name: undefined,
      project_id: undefined,
      session_id: undefined,
      cost_status: undefined,
    };
    setDraft({});
    setPage(0);
    setFilter(next);
  };

  const persistViews = (views: SavedView[]) => {
    setSavedViews(views);
    try {
      localStorage.setItem(SAVED_VIEWS_KEY, JSON.stringify(views));
    } catch {
      // Ignore persistence failures.
    }
  };

  const saveCurrentView = () => {
    const name = viewName.trim();
    if (!name) return;
    const savedFilter: DashboardFilterV2 = {
      ...filter,
      provider: draft.provider?.trim() || undefined,
      model_name: draft.model_name?.trim() || undefined,
      project_id: draft.project_id?.trim() || undefined,
      session_id: draft.session_id?.trim() || undefined,
      cost_status: draft.cost_status,
    };
    const next = [...savedViews.filter((v) => v.name !== name), { name, filter: savedFilter }];
    persistViews(next);
    setViewName('');
  };

  const applySavedView = (name: string) => {
    const view = savedViews.find((v) => v.name === name);
    if (!view) return;
    setDraft({
      provider: view.filter.provider,
      model_name: view.filter.model_name,
      project_id: view.filter.project_id,
      session_id: view.filter.session_id,
      cost_status: view.filter.cost_status,
    });
    setPage(0);
    setFilter(view.filter);
  };

  const paging = useMemo(() => {
    if (totalRecords === 0) {
      return { from: 0, to: 0 };
    }
    const from = page * pageSize + 1;
    const to = Math.min((page + 1) * pageSize, totalRecords);
    return { from, to };
  }, [page, pageSize, totalRecords]);

  const noMoreNext = paging.to >= totalRecords;

  return (
    <div
      className={clsx(
        'bg-white dark:bg-gray-900 rounded-xl',
        'border border-gray-200 dark:border-gray-800',
        'overflow-hidden',
      )}
    >
      {/* Table Header */}
      <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-800">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">{t('table.title', 'Usage Records')}</h3>
      </div>

      {/* Filters */}
      <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-800 bg-gray-50/70 dark:bg-gray-900/40">
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-5 gap-2">
          <input
            value={draft.provider || ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, provider: e.target.value }))}
            placeholder={t('filters.provider', 'Provider')}
            className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          />
          <input
            value={draft.model_name || ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, model_name: e.target.value }))}
            placeholder={t('filters.model', 'Model')}
            className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          />
          <input
            value={draft.project_id || ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, project_id: e.target.value }))}
            placeholder={t('filters.project', 'Project ID')}
            className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          />
          <input
            value={draft.session_id || ''}
            onChange={(e) => setDraft((prev) => ({ ...prev, session_id: e.target.value }))}
            placeholder={t('filters.session', 'Session ID')}
            className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          />
          <select
            value={draft.cost_status || ''}
            onChange={(e) =>
              setDraft((prev) => ({
                ...prev,
                cost_status: (e.target.value || undefined) as CostStatus | undefined,
              }))
            }
            className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          >
            <option value="">{t('filters.costStatusAll', 'All Cost Status')}</option>
            <option value="exact">{t('filters.costStatusExact', 'Exact')}</option>
            <option value="estimated">{t('filters.costStatusEstimated', 'Estimated')}</option>
            <option value="missing">{t('filters.costStatusMissing', 'Missing')}</option>
          </select>
        </div>
        <div className="mt-3 flex items-center gap-2">
          <button
            onClick={applyFilter}
            className="px-3 py-1.5 rounded-lg text-sm bg-primary-600 text-white hover:bg-primary-700"
          >
            {t('filters.apply', 'Apply Filters')}
          </button>
          <button
            onClick={clearAdvancedFilter}
            className="px-3 py-1.5 rounded-lg text-sm bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700"
          >
            {t('filters.clear', 'Clear')}
          </button>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <input
            value={viewName}
            onChange={(e) => setViewName(e.target.value)}
            placeholder={t('filters.viewName', 'View name')}
            className="px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          />
          <button
            onClick={saveCurrentView}
            className="px-3 py-1.5 rounded-lg text-sm bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700"
          >
            {t('filters.saveView', 'Save View')}
          </button>
          <select
            defaultValue=""
            onChange={(e) => {
              if (e.target.value) applySavedView(e.target.value);
            }}
            className="px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
          >
            <option value="">{t('filters.savedViews', 'Saved Views')}</option>
            {savedViews.map((view) => (
              <option key={view.name} value={view.name}>
                {view.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 dark:bg-gray-800/50">
            <tr className="text-left text-gray-500 dark:text-gray-400">
              <th className="px-6 py-3 font-medium">{t('table.timestamp', 'Timestamp')}</th>
              <th className="px-6 py-3 font-medium">{t('table.model', 'Model')}</th>
              <th className="px-6 py-3 font-medium">{t('table.provider', 'Provider')}</th>
              <th className="px-6 py-3 font-medium">{t('table.project', 'Project')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.inputTokens', 'Input')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.outputTokens', 'Output')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.thinkingTokens', 'Thinking')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.cacheTokens', 'Cache R/W')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.cost', 'Cost')}</th>
              <th className="px-6 py-3 font-medium">{t('table.costStatus', 'Cost Status')}</th>
              <th className="px-6 py-3 font-medium">{t('table.session', 'Session')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
            {recordsLoading && records.length === 0 ? (
              Array.from({ length: 6 }).map((_, i) => (
                <tr key={i} className="animate-pulse">
                  {Array.from({ length: 11 }).map((_, j) => (
                    <td key={j} className="px-6 py-4">
                      <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
                    </td>
                  ))}
                </tr>
              ))
            ) : records.length === 0 ? (
              <tr>
                <td colSpan={11} className="px-6 py-12 text-center text-gray-500 dark:text-gray-400">
                  {t('table.noRecords', 'No usage records found')}
                </td>
              </tr>
            ) : (
              records.map((record) => (
                <tr
                  key={record.event_id}
                  className="text-gray-900 dark:text-white hover:bg-gray-50 dark:hover:bg-gray-800/50"
                >
                  <td className="px-6 py-4 whitespace-nowrap text-xs text-gray-500 dark:text-gray-400">
                    {formatTimestamp(record.timestamp)}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap font-medium">{record.model_name}</td>
                  <td className="px-6 py-4 whitespace-nowrap capitalize">{record.provider}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-xs text-gray-500 dark:text-gray-400">
                    {record.project_id || '-'}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-right">{formatTokens(record.input_tokens)}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-right">{formatTokens(record.output_tokens)}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-right text-gray-500 dark:text-gray-400">
                    {record.thinking_tokens > 0 ? formatTokens(record.thinking_tokens) : '-'}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-right text-gray-500 dark:text-gray-400">
                    {record.cache_read_tokens > 0 || record.cache_creation_tokens > 0
                      ? `${formatTokens(record.cache_read_tokens)} / ${formatTokens(record.cache_creation_tokens)}`
                      : '-'}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-right font-medium">
                    {formatCost(record.cost_microdollars)}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    <span
                      className={clsx(
                        'px-2 py-0.5 rounded-full text-xs',
                        record.cost_status === 'exact' &&
                          'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300',
                        record.cost_status === 'estimated' &&
                          'bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300',
                        record.cost_status === 'missing' &&
                          'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300',
                      )}
                    >
                      {record.cost_status}
                    </span>
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-xs text-gray-500 dark:text-gray-400">
                    {record.session_id ? (
                      <span title={record.session_id}>{record.session_id.substring(0, 12)}...</span>
                    ) : (
                      '-'
                    )}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="px-6 py-4 border-t border-gray-200 dark:border-gray-800 flex items-center justify-between">
        <div className="text-sm text-gray-500 dark:text-gray-400">
          {t('table.showing', 'Showing')} {paging.from} - {paging.to} {t('table.of', 'of')} {totalRecords}{' '}
          {t('table.records', 'records')}
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setPage(Math.max(0, page - 1))}
            disabled={page === 0}
            className={clsx(
              'px-3 py-1.5 rounded-lg text-sm',
              'border border-gray-200 dark:border-gray-700',
              'hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {t('table.previous', 'Previous')}
          </button>
          <button
            onClick={() => setPage(page + 1)}
            disabled={noMoreNext}
            className={clsx(
              'px-3 py-1.5 rounded-lg text-sm',
              'border border-gray-200 dark:border-gray-700',
              'hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {t('table.next', 'Next')}
          </button>
        </div>
      </div>
    </div>
  );
}

export default UsageTable;
