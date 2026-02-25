/**
 * Usage Table Component
 *
 * Displays detailed usage records in a paginated table.
 */

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useAnalyticsStore, formatCost, formatTokens } from '../../store/analytics';

export function UsageTable() {
  const { t } = useTranslation('analytics');
  const [page, setPage] = useState(0);
  const pageSize = 20;

  const { records, isLoading, fetchRecords } = useAnalyticsStore();

  useEffect(() => {
    fetchRecords(pageSize, page * pageSize);
  }, [fetchRecords, page]);

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

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

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 dark:bg-gray-800/50">
            <tr className="text-left text-gray-500 dark:text-gray-400">
              <th className="px-6 py-3 font-medium">{t('table.timestamp', 'Timestamp')}</th>
              <th className="px-6 py-3 font-medium">{t('table.model', 'Model')}</th>
              <th className="px-6 py-3 font-medium">{t('table.provider', 'Provider')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.inputTokens', 'Input')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.outputTokens', 'Output')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.thinkingTokens', 'Thinking')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.cacheTokens', 'Cache R/W')}</th>
              <th className="px-6 py-3 font-medium text-right">{t('table.cost', 'Cost')}</th>
              <th className="px-6 py-3 font-medium">{t('table.session', 'Session')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
            {isLoading && records.length === 0 ? (
              // Loading skeleton
              Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="animate-pulse">
                  {Array.from({ length: 9 }).map((_, j) => (
                    <td key={j} className="px-6 py-4">
                      <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
                    </td>
                  ))}
                </tr>
              ))
            ) : records.length === 0 ? (
              <tr>
                <td colSpan={9} className="px-6 py-12 text-center text-gray-500 dark:text-gray-400">
                  {t('table.noRecords', 'No usage records found')}
                </td>
              </tr>
            ) : (
              records.map((record) => (
                <tr
                  key={record.id}
                  className="text-gray-900 dark:text-white hover:bg-gray-50 dark:hover:bg-gray-800/50"
                >
                  <td className="px-6 py-4 whitespace-nowrap text-xs text-gray-500 dark:text-gray-400">
                    {formatTimestamp(record.timestamp)}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap font-medium">{record.model_name}</td>
                  <td className="px-6 py-4 whitespace-nowrap capitalize">{record.provider}</td>
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
                  <td className="px-6 py-4 whitespace-nowrap text-xs text-gray-500 dark:text-gray-400">
                    {record.session_id ? (
                      <span title={record.session_id}>{record.session_id.substring(0, 8)}...</span>
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
          {t('table.showing', 'Showing')} {page * pageSize + 1} -{' '}
          {Math.min((page + 1) * pageSize, page * pageSize + records.length)} {t('table.records', 'records')}
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
            disabled={records.length < pageSize}
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
