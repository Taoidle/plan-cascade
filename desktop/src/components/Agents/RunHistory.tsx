/**
 * RunHistory Component
 *
 * Displays the execution history for an agent.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon, ClockIcon, DownloadIcon } from '@radix-ui/react-icons';
import type { AgentRun, AgentRunList } from '../../types/agent';
import { getStatusColor, getStatusBgColor, formatDuration, formatTokens } from '../../types/agent';

interface RunHistoryProps {
  /** Run history data */
  history: AgentRunList | null;
  /** Whether data is loading */
  loading?: boolean;
  /** Callback to load more runs */
  onLoadMore?: () => void;
  /** Callback to export history */
  onExport?: () => void;
}

export function RunHistory({ history, loading, onLoadMore, onExport }: RunHistoryProps) {
  const { t } = useTranslation();
  const [expandedRuns, setExpandedRuns] = useState<Set<string>>(new Set());

  const toggleRun = (runId: string) => {
    setExpandedRuns((prev) => {
      const next = new Set(prev);
      if (next.has(runId)) {
        next.delete(runId);
      } else {
        next.add(runId);
      }
      return next;
    });
  };

  if (loading && (!history || history.runs.length === 0)) {
    return (
      <div className="space-y-2">
        <RunHistorySkeleton />
        <RunHistorySkeleton />
        <RunHistorySkeleton />
      </div>
    );
  }

  if (!history || history.runs.length === 0) {
    return (
      <div className="text-center py-8">
        <ClockIcon className="w-8 h-8 mx-auto mb-2 text-gray-400" />
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('agents.noRunHistory')}
        </p>
      </div>
    );
  }

  const hasMore = history.offset + history.runs.length < history.total;

  return (
    <div className="space-y-2">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {t('agents.runHistory')} ({history.total})
        </h3>
        {onExport && (
          <button
            onClick={onExport}
            className={clsx(
              'flex items-center gap-1 px-2 py-1 rounded text-xs',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800'
            )}
          >
            <DownloadIcon className="w-3 h-3" />
            {t('agents.exportHistory')}
          </button>
        )}
      </div>

      {/* Run List */}
      {history.runs.map((run) => (
        <RunHistoryItem
          key={run.id}
          run={run}
          expanded={expandedRuns.has(run.id)}
          onToggle={() => toggleRun(run.id)}
        />
      ))}

      {/* Load More */}
      {hasMore && (
        <button
          onClick={onLoadMore}
          disabled={loading}
          className={clsx(
            'w-full py-2 rounded-md text-sm',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'disabled:opacity-50'
          )}
        >
          {loading ? t('common.loading') : t('common.loadMore')}
        </button>
      )}
    </div>
  );
}

interface RunHistoryItemProps {
  run: AgentRun;
  expanded: boolean;
  onToggle: () => void;
}

function RunHistoryItem({ run, expanded, onToggle }: RunHistoryItemProps) {
  const { t } = useTranslation();

  const timestamp = run.created_at
    ? new Date(run.created_at).toLocaleString()
    : '-';

  const inputPreview =
    run.input.length > 100 ? run.input.substring(0, 100) + '...' : run.input;

  return (
    <div
      className={clsx(
        'rounded-md border transition-colors',
        'border-gray-200 dark:border-gray-700',
        'bg-white dark:bg-gray-800'
      )}
    >
      {/* Header Row */}
      <button
        onClick={onToggle}
        className={clsx(
          'w-full flex items-center gap-3 p-3 text-left',
          'hover:bg-gray-50 dark:hover:bg-gray-750'
        )}
      >
        {/* Expand Icon */}
        {expanded ? (
          <ChevronDownIcon className="w-4 h-4 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRightIcon className="w-4 h-4 text-gray-400 flex-shrink-0" />
        )}

        {/* Status Badge */}
        <span
          className={clsx(
            'px-2 py-0.5 rounded text-xs font-medium flex-shrink-0',
            getStatusBgColor(run.status),
            getStatusColor(run.status)
          )}
        >
          {run.status}
        </span>

        {/* Input Preview */}
        <span className="flex-1 text-sm text-gray-700 dark:text-gray-300 truncate">
          {inputPreview}
        </span>

        {/* Duration & Time */}
        <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400 flex-shrink-0">
          {run.duration_ms && (
            <span className="flex items-center gap-1">
              <ClockIcon className="w-3 h-3" />
              {formatDuration(run.duration_ms)}
            </span>
          )}
          <span>{timestamp}</span>
        </div>
      </button>

      {/* Expanded Content */}
      {expanded && (
        <div className="px-3 pb-3 pt-0 border-t border-gray-100 dark:border-gray-700">
          {/* Input */}
          <div className="mt-3">
            <label className="text-xs font-medium text-gray-500 dark:text-gray-400">
              {t('agents.input')}
            </label>
            <div
              className={clsx(
                'mt-1 p-2 rounded text-sm font-mono whitespace-pre-wrap',
                'bg-gray-50 dark:bg-gray-900',
                'text-gray-800 dark:text-gray-200'
              )}
            >
              {run.input}
            </div>
          </div>

          {/* Output */}
          {run.output && (
            <div className="mt-3">
              <label className="text-xs font-medium text-gray-500 dark:text-gray-400">
                {t('agents.output')}
              </label>
              <div
                className={clsx(
                  'mt-1 p-2 rounded text-sm font-mono whitespace-pre-wrap max-h-64 overflow-y-auto',
                  'bg-gray-50 dark:bg-gray-900',
                  'text-gray-800 dark:text-gray-200'
                )}
              >
                {run.output}
              </div>
            </div>
          )}

          {/* Error */}
          {run.error && (
            <div className="mt-3">
              <label className="text-xs font-medium text-red-500 dark:text-red-400">
                {t('agents.error')}
              </label>
              <div
                className={clsx(
                  'mt-1 p-2 rounded text-sm font-mono',
                  'bg-red-50 dark:bg-red-900/20',
                  'text-red-700 dark:text-red-300'
                )}
              >
                {run.error}
              </div>
            </div>
          )}

          {/* Token Usage */}
          {(run.input_tokens || run.output_tokens) && (
            <div className="mt-3 flex items-center gap-4 text-xs text-gray-500 dark:text-gray-400">
              <span>
                {t('agents.inputTokens')}: {formatTokens(run.input_tokens)}
              </span>
              <span>
                {t('agents.outputTokens')}: {formatTokens(run.output_tokens)}
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function RunHistorySkeleton() {
  return (
    <div className="p-3 rounded-md border border-gray-200 dark:border-gray-700 animate-pulse">
      <div className="flex items-center gap-3">
        <div className="w-4 h-4 bg-gray-200 dark:bg-gray-700 rounded" />
        <div className="w-16 h-5 bg-gray-200 dark:bg-gray-700 rounded" />
        <div className="flex-1 h-4 bg-gray-100 dark:bg-gray-800 rounded" />
        <div className="w-24 h-4 bg-gray-100 dark:bg-gray-800 rounded" />
      </div>
    </div>
  );
}

export default RunHistory;
