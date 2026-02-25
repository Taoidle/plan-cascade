/**
 * Overview Cards Component
 *
 * Displays summary statistics cards showing total cost, tokens, and requests
 * with comparison to previous period.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { DashboardSummary } from '../../store/analytics';
import { formatCost, formatTokens, formatChange } from '../../store/analytics';

interface OverviewCardsProps {
  summary: DashboardSummary;
  isLoading?: boolean;
}

export function OverviewCards({ summary, isLoading }: OverviewCardsProps) {
  const { t } = useTranslation('analytics');

  const cards = [
    {
      title: t('cards.totalCost', 'Total Cost'),
      value: formatCost(summary.current_period.total_cost_microdollars),
      change: summary.cost_change_percent,
      icon: (
        <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      ),
      colorClass: 'text-green-600 dark:text-green-400',
      bgClass: 'bg-green-50 dark:bg-green-900/20',
    },
    {
      title: t('cards.totalTokens', 'Total Tokens'),
      value: formatTokens(summary.current_period.total_input_tokens + summary.current_period.total_output_tokens),
      change: summary.tokens_change_percent,
      icon: (
        <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
          />
        </svg>
      ),
      colorClass: 'text-blue-600 dark:text-blue-400',
      bgClass: 'bg-blue-50 dark:bg-blue-900/20',
    },
    {
      title: t('cards.totalRequests', 'Total Requests'),
      value: summary.current_period.request_count.toLocaleString(),
      change: summary.requests_change_percent,
      icon: (
        <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      ),
      colorClass: 'text-purple-600 dark:text-purple-400',
      bgClass: 'bg-purple-50 dark:bg-purple-900/20',
    },
    {
      title: t('cards.avgCostPerRequest', 'Avg Cost/Request'),
      value: formatCost(summary.current_period.avg_cost_per_request),
      change: null, // No comparison for average
      icon: (
        <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z"
          />
        </svg>
      ),
      colorClass: 'text-orange-600 dark:text-orange-400',
      bgClass: 'bg-orange-50 dark:bg-orange-900/20',
    },
  ];

  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      {cards.map((card) => (
        <div
          key={card.title}
          className={clsx(
            'bg-white dark:bg-gray-900 rounded-xl',
            'border border-gray-200 dark:border-gray-800',
            'p-6',
            'transition-shadow hover:shadow-md',
            isLoading && 'animate-pulse',
          )}
        >
          <div className="flex items-start justify-between">
            <div className={clsx('p-2 rounded-lg', card.bgClass)}>
              <div className={card.colorClass}>{card.icon}</div>
            </div>

            {card.change !== null && <ChangeIndicator value={card.change} />}
          </div>

          <div className="mt-4">
            <p className="text-sm font-medium text-gray-500 dark:text-gray-400">{card.title}</p>
            <p className="mt-1 text-2xl font-semibold text-gray-900 dark:text-white">
              {isLoading ? '---' : card.value}
            </p>
          </div>
        </div>
      ))}
    </div>
  );
}

interface ChangeIndicatorProps {
  value: number;
}

function ChangeIndicator({ value }: ChangeIndicatorProps) {
  const isPositive = value >= 0;
  const isNeutral = Math.abs(value) < 0.1;

  if (isNeutral) {
    return (
      <span
        className={clsx(
          'inline-flex items-center px-2 py-1 rounded-full text-xs font-medium',
          'bg-gray-100 dark:bg-gray-800',
          'text-gray-600 dark:text-gray-400',
        )}
      >
        0%
      </span>
    );
  }

  return (
    <span
      className={clsx(
        'inline-flex items-center px-2 py-1 rounded-full text-xs font-medium',
        isPositive
          ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
          : 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
      )}
    >
      {isPositive ? (
        <svg className="w-3 h-3 mr-1" fill="currentColor" viewBox="0 0 20 20">
          <path
            fillRule="evenodd"
            d="M5.293 9.707a1 1 0 010-1.414l4-4a1 1 0 011.414 0l4 4a1 1 0 01-1.414 1.414L11 7.414V15a1 1 0 11-2 0V7.414L6.707 9.707a1 1 0 01-1.414 0z"
            clipRule="evenodd"
          />
        </svg>
      ) : (
        <svg className="w-3 h-3 mr-1" fill="currentColor" viewBox="0 0 20 20">
          <path
            fillRule="evenodd"
            d="M14.707 10.293a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 111.414-1.414L9 12.586V5a1 1 0 012 0v7.586l2.293-2.293a1 1 0 011.414 0z"
            clipRule="evenodd"
          />
        </svg>
      )}
      {formatChange(value)}
    </span>
  );
}

export default OverviewCards;
