/**
 * AgentCard Component
 *
 * Displays a single agent with stats and action buttons.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { PlayIcon, Pencil1Icon, TrashIcon, ClockIcon } from '@radix-ui/react-icons';
import type { AgentWithStats } from '../../types/agent';
import { formatDuration } from '../../types/agent';

interface AgentCardProps {
  agent: AgentWithStats;
  onRun: () => void;
  onEdit: () => void;
  onDelete: () => void;
  isSelected?: boolean;
}

export function AgentCard({
  agent,
  onRun,
  onEdit,
  onDelete,
  isSelected = false,
}: AgentCardProps) {
  const { t } = useTranslation();
  const { stats } = agent;

  return (
    <div
      className={clsx(
        'p-4 rounded-lg border transition-all cursor-pointer',
        isSelected
          ? 'bg-primary-50 dark:bg-primary-900/20 border-primary-500'
          : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
      )}
    >
      {/* Header Row */}
      <div className="flex items-start justify-between mb-3">
        <div className="flex-1 min-w-0">
          {/* Agent Name */}
          <h3 className="font-semibold text-gray-900 dark:text-white truncate">
            {agent.name}
          </h3>

          {/* Model Badge */}
          <div className="flex items-center gap-2 mt-1">
            <span
              className={clsx(
                'px-1.5 py-0.5 rounded text-xs font-medium',
                'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300'
              )}
            >
              {agent.model.split('-').slice(0, 2).join(' ')}
            </span>

            {/* Success Rate Badge */}
            {stats.total_runs > 0 && (
              <span
                className={clsx(
                  'px-1.5 py-0.5 rounded text-xs font-medium',
                  stats.success_rate >= 80
                    ? 'bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-300'
                    : stats.success_rate >= 50
                    ? 'bg-yellow-100 dark:bg-yellow-900/50 text-yellow-700 dark:text-yellow-300'
                    : 'bg-red-100 dark:bg-red-900/50 text-red-700 dark:text-red-300'
                )}
              >
                {stats.success_rate.toFixed(0)}%
              </span>
            )}
          </div>
        </div>

        {/* Tools Count */}
        {agent.allowed_tools.length > 0 && (
          <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
            {agent.allowed_tools.length} {t('agents.tools', { count: agent.allowed_tools.length })}
          </span>
        )}
      </div>

      {/* Description */}
      {agent.description && (
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-3 line-clamp-2">
          {agent.description}
        </p>
      )}

      {/* Stats Row */}
      <div className="flex items-center gap-4 mb-3 text-xs text-gray-500 dark:text-gray-400">
        <span>{stats.total_runs} {t('agents.runs', { count: stats.total_runs })}</span>
        {stats.avg_duration_ms > 0 && (
          <span className="flex items-center gap-1">
            <ClockIcon className="w-3 h-3" />
            {formatDuration(stats.avg_duration_ms)}
          </span>
        )}
        {stats.last_run_at && (
          <span className="truncate">
            {t('agents.lastRun')}: {new Date(stats.last_run_at).toLocaleDateString()}
          </span>
        )}
      </div>

      {/* Action Buttons */}
      <div className="flex items-center gap-2">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRun();
          }}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            'bg-primary-100 dark:bg-primary-900/50',
            'text-primary-700 dark:text-primary-300',
            'hover:bg-primary-200 dark:hover:bg-primary-800',
            'text-xs font-medium transition-colors'
          )}
        >
          <PlayIcon className="w-3 h-3" />
          <span>{t('agents.run')}</span>
        </button>

        <button
          onClick={(e) => {
            e.stopPropagation();
            onEdit();
          }}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            'bg-gray-100 dark:bg-gray-700',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-600',
            'text-xs font-medium transition-colors'
          )}
        >
          <Pencil1Icon className="w-3 h-3" />
          <span>{t('agents.edit')}</span>
        </button>

        <button
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            'bg-red-100 dark:bg-red-900/50',
            'text-red-700 dark:text-red-300',
            'hover:bg-red-200 dark:hover:bg-red-800',
            'text-xs font-medium transition-colors ml-auto'
          )}
        >
          <TrashIcon className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}

export function AgentCardSkeleton() {
  return (
    <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700 animate-pulse">
      <div className="flex items-start justify-between mb-3">
        <div>
          <div className="h-5 w-32 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
          <div className="flex gap-2">
            <div className="h-5 w-16 bg-gray-100 dark:bg-gray-800 rounded" />
            <div className="h-5 w-10 bg-gray-100 dark:bg-gray-800 rounded" />
          </div>
        </div>
      </div>
      <div className="h-4 w-48 bg-gray-100 dark:bg-gray-800 rounded mb-3" />
      <div className="flex gap-2">
        <div className="h-7 w-14 bg-gray-100 dark:bg-gray-800 rounded" />
        <div className="h-7 w-12 bg-gray-100 dark:bg-gray-800 rounded" />
      </div>
    </div>
  );
}

export default AgentCard;
