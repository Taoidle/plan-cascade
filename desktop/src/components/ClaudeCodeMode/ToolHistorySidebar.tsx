/**
 * ToolHistorySidebar Component
 *
 * Shows the history of all tool calls in the current conversation.
 * Enhanced with advanced filtering, text search, comprehensive statistics,
 * success rate tracking, duration analytics, and tool usage patterns.
 *
 * Story-007: ToolHistorySidebar enhancement with filtering, search, and statistics
 */

import { useState, useMemo, useCallback, useEffect, KeyboardEvent } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  FileTextIcon,
  Pencil1Icon,
  CodeIcon,
  MagnifyingGlassIcon,
  FileIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  ReloadIcon,
  GlobeIcon,
  MixerHorizontalIcon,
  Cross2Icon,
  ChevronDownIcon,
  ChevronUpIcon,
  BarChartIcon,
  ClockIcon,
  TrashIcon,
  DownloadIcon,
  PlayIcon,
} from '@radix-ui/react-icons';
import { useClaudeCodeStore, ToolType, ToolCall, ToolCallStatus } from '../../store/claudeCode';
import { ExecutionTimeline } from './ExecutionTimeline';

// ============================================================================
// Types
// ============================================================================

type StatusFilter = 'all' | ToolCallStatus;

interface ToolStatistics {
  total: number;
  successRate: number;
  averageDuration: number;
  byTool: Record<string, { count: number; successRate: number; avgDuration: number }>;
  byStatus: Record<string, number>;
  mostUsed: { tool: string; count: number }[];
  failedOperations: ToolCall[];
  totalDuration: number;
}

// ============================================================================
// Custom Hook: useToolStatistics
// ============================================================================

function useToolStatistics(toolCalls: ToolCall[]): ToolStatistics {
  return useMemo(() => {
    const byTool: Record<string, { count: number; success: number; totalDuration: number }> = {};
    const byStatus: Record<string, number> = {
      pending: 0,
      executing: 0,
      completed: 0,
      failed: 0,
    };
    const failedOperations: ToolCall[] = [];
    let totalSuccess = 0;
    let totalDuration = 0;

    toolCalls.forEach((tc) => {
      // By status
      byStatus[tc.status] = (byStatus[tc.status] || 0) + 1;

      // By tool
      if (!byTool[tc.name]) {
        byTool[tc.name] = { count: 0, success: 0, totalDuration: 0 };
      }
      byTool[tc.name].count++;

      if (tc.status === 'completed' && tc.result?.success !== false) {
        byTool[tc.name].success++;
        totalSuccess++;
      }

      if (tc.status === 'failed' || tc.result?.success === false) {
        failedOperations.push(tc);
      }

      if (tc.duration) {
        byTool[tc.name].totalDuration += tc.duration;
        totalDuration += tc.duration;
      }
    });

    // Calculate derived statistics
    const total = toolCalls.length;
    const successRate = total > 0 ? (totalSuccess / total) * 100 : 0;
    const averageDuration = total > 0 ? totalDuration / total : 0;

    // Most used tools
    const mostUsed = Object.entries(byTool)
      .map(([tool, stats]) => ({ tool, count: stats.count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 5);

    // Calculate per-tool statistics
    const byToolStats: Record<string, { count: number; successRate: number; avgDuration: number }> = {};
    Object.entries(byTool).forEach(([tool, stats]) => {
      byToolStats[tool] = {
        count: stats.count,
        successRate: stats.count > 0 ? (stats.success / stats.count) * 100 : 0,
        avgDuration: stats.count > 0 ? stats.totalDuration / stats.count : 0,
      };
    });

    return {
      total,
      successRate,
      averageDuration,
      byTool: byToolStats,
      byStatus,
      mostUsed,
      failedOperations,
      totalDuration,
    };
  }, [toolCalls]);
}

// ============================================================================
// StatisticsPanel Component
// ============================================================================

interface StatisticsPanelProps {
  stats: ToolStatistics;
  onRetryFailed?: (toolCall: ToolCall) => void;
}

function StatisticsPanel({ stats, onRetryFailed }: StatisticsPanelProps) {
  const [showFailedList, setShowFailedList] = useState(false);

  return (
    <div className="space-y-4">
      {/* Summary stats */}
      <div className="grid grid-cols-2 gap-3">
        <div className="bg-white dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
          <div className="text-2xl font-bold text-gray-900 dark:text-white">
            {stats.total}
          </div>
          <div className="text-xs text-gray-500">Total Calls</div>
        </div>
        <div className="bg-white dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
          <div className={clsx(
            'text-2xl font-bold',
            stats.successRate >= 80 ? 'text-green-600 dark:text-green-400' :
            stats.successRate >= 50 ? 'text-yellow-600 dark:text-yellow-400' :
            'text-red-600 dark:text-red-400'
          )}>
            {stats.successRate.toFixed(0)}%
          </div>
          <div className="text-xs text-gray-500">Success Rate</div>
        </div>
        <div className="bg-white dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
          <div className="text-2xl font-bold text-gray-900 dark:text-white">
            {formatDuration(stats.averageDuration)}
          </div>
          <div className="text-xs text-gray-500">Avg Duration</div>
        </div>
        <div className="bg-white dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
          <div className="text-2xl font-bold text-gray-900 dark:text-white">
            {formatDuration(stats.totalDuration)}
          </div>
          <div className="text-xs text-gray-500">Total Time</div>
        </div>
      </div>

      {/* Duration by tool (bar chart) */}
      {stats.mostUsed.length > 0 && (
        <div className="bg-white dark:bg-gray-800 rounded-lg p-3 border border-gray-200 dark:border-gray-700">
          <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
            Tool Usage
          </h4>
          <div className="space-y-2">
            {stats.mostUsed.map(({ tool, count }) => {
              const percentage = (count / stats.total) * 100;
              const toolStats = stats.byTool[tool];
              const config = getToolConfig(tool as ToolType);

              return (
                <div key={tool} className="space-y-1">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center gap-1.5">
                      <config.Icon className={clsx('w-3 h-3', config.iconColor)} />
                      <span className="text-gray-700 dark:text-gray-300">{tool}</span>
                    </div>
                    <span className="text-gray-500">
                      {count} ({percentage.toFixed(0)}%)
                    </span>
                  </div>
                  <div className="h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                    <div
                      className={clsx('h-full rounded-full', config.barColor)}
                      style={{ width: `${percentage}%` }}
                    />
                  </div>
                  <div className="flex justify-between text-[10px] text-gray-400">
                    <span>{toolStats.successRate.toFixed(0)}% success</span>
                    <span>avg {formatDuration(toolStats.avgDuration)}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Failed operations */}
      {stats.failedOperations.length > 0 && (
        <div className="bg-red-50 dark:bg-red-900/20 rounded-lg border border-red-200 dark:border-red-800">
          <button
            onClick={() => setShowFailedList(!showFailedList)}
            className="w-full flex items-center justify-between px-3 py-2 text-left"
          >
            <div className="flex items-center gap-2 text-red-700 dark:text-red-400">
              <CrossCircledIcon className="w-4 h-4" />
              <span className="text-sm font-medium">
                {stats.failedOperations.length} Failed Operation{stats.failedOperations.length !== 1 ? 's' : ''}
              </span>
            </div>
            {showFailedList ? (
              <ChevronUpIcon className="w-4 h-4 text-red-500" />
            ) : (
              <ChevronDownIcon className="w-4 h-4 text-red-500" />
            )}
          </button>

          {showFailedList && (
            <div className="px-3 pb-3 space-y-2">
              {stats.failedOperations.map((tc) => (
                <div
                  key={tc.id}
                  className="flex items-center justify-between p-2 bg-white dark:bg-gray-800 rounded border border-red-200 dark:border-red-800"
                >
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-gray-900 dark:text-white">
                      {tc.name}
                    </div>
                    <div className="text-xs text-gray-500 truncate">
                      {getToolSummary(tc)}
                    </div>
                  </div>
                  {onRetryFailed && (
                    <button
                      onClick={() => onRetryFailed(tc)}
                      className={clsx(
                        'flex items-center gap-1 px-2 py-1 rounded text-xs',
                        'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400',
                        'hover:bg-red-200 dark:hover:bg-red-900/50',
                        'transition-colors'
                      )}
                    >
                      <PlayIcon className="w-3 h-3" />
                      Retry
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// SearchInput Component
// ============================================================================

interface SearchInputProps {
  value: string;
  onChange: (value: string) => void;
  onClear: () => void;
}

function SearchInput({ value, onChange, onClear }: SearchInputProps) {
  return (
    <div className="relative">
      <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Search tool calls..."
        className={clsx(
          'w-full pl-8 pr-8 py-2 rounded-lg text-sm',
          'bg-white dark:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
          'focus:outline-none focus:ring-2 focus:ring-primary-500',
          'placeholder-gray-400'
        )}
      />
      {value && (
        <button
          onClick={onClear}
          className="absolute right-2.5 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
        >
          <Cross2Icon className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}

// ============================================================================
// ToolHistorySidebar Component
// ============================================================================

interface ToolHistorySidebarProps {
  onToolClick?: (toolCallId: string) => void;
  onClose?: () => void;
}

export function ToolHistorySidebar({ onToolClick, onClose }: ToolHistorySidebarProps) {
  const { t } = useTranslation('claudeCode');
  const { toolCallHistory, setToolFilter, clearConversation } = useClaudeCodeStore();

  // Local state
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [selectedTools, setSelectedTools] = useState<Set<ToolType | 'all'>>(new Set(['all']));
  const [showStats, setShowStats] = useState(false);
  const [showTimeline, setShowTimeline] = useState(false);
  const [focusedIndex, setFocusedIndex] = useState(-1);

  // Calculate statistics
  const stats = useToolStatistics(toolCallHistory);

  // Filter tool calls
  const filteredToolCalls = useMemo(() => {
    return toolCallHistory.filter((tc) => {
      // Tool type filter (multi-select)
      if (!selectedTools.has('all') && !selectedTools.has(tc.name)) {
        return false;
      }

      // Status filter
      if (statusFilter !== 'all' && tc.status !== statusFilter) {
        return false;
      }

      // Search filter
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesName = tc.name.toLowerCase().includes(query);
        const matchesPath = tc.parameters.file_path?.toLowerCase().includes(query);
        const matchesCommand = tc.parameters.command?.toLowerCase().includes(query);
        const matchesPattern = tc.parameters.pattern?.toLowerCase().includes(query);

        if (!matchesName && !matchesPath && !matchesCommand && !matchesPattern) {
          return false;
        }
      }

      return true;
    });
  }, [toolCallHistory, selectedTools, statusFilter, searchQuery]);

  // Tool type options
  const toolTypes: ToolType[] = ['Read', 'Write', 'Edit', 'Bash', 'Glob', 'Grep', 'WebFetch', 'WebSearch'];

  // Handle tool type toggle (multi-select)
  const handleToolToggle = useCallback((tool: ToolType | 'all') => {
    setSelectedTools(prev => {
      const next = new Set(prev);

      if (tool === 'all') {
        // If clicking "All", select only "All"
        return new Set(['all']);
      }

      // Remove "all" if selecting a specific tool
      next.delete('all');

      if (next.has(tool)) {
        next.delete(tool);
        // If nothing selected, select "all"
        if (next.size === 0) {
          return new Set(['all']);
        }
      } else {
        next.add(tool);
      }

      return next;
    });
  }, []);

  // Clear all filters
  const handleClearFilters = useCallback(() => {
    setSearchQuery('');
    setStatusFilter('all');
    setSelectedTools(new Set(['all']));
    setToolFilter('all');
  }, [setToolFilter]);

  // Export filtered results
  const handleExport = useCallback(() => {
    const data = filteredToolCalls.map(tc => ({
      id: tc.id,
      name: tc.name,
      status: tc.status,
      parameters: tc.parameters,
      result: tc.result,
      duration: tc.duration,
      startedAt: tc.startedAt,
      completedAt: tc.completedAt,
    }));

    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `tool-history-${new Date().toISOString().slice(0, 10)}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }, [filteredToolCalls]);

  // Keyboard navigation
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setFocusedIndex(prev => Math.min(prev + 1, filteredToolCalls.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setFocusedIndex(prev => Math.max(prev - 1, 0));
    } else if (e.key === 'Enter' && focusedIndex >= 0) {
      const toolCall = filteredToolCalls[focusedIndex];
      if (toolCall) {
        onToolClick?.(toolCall.id);
      }
    }
  }, [filteredToolCalls, focusedIndex, onToolClick]);

  // Load filter preferences from localStorage
  useEffect(() => {
    const savedFilters = localStorage.getItem('toolHistoryFilters');
    if (savedFilters) {
      try {
        const { tools, status } = JSON.parse(savedFilters);
        if (tools) setSelectedTools(new Set(tools));
        if (status) setStatusFilter(status);
      } catch (e) {
        // Ignore invalid data
      }
    }
  }, []);

  // Save filter preferences
  useEffect(() => {
    localStorage.setItem('toolHistoryFilters', JSON.stringify({
      tools: Array.from(selectedTools),
      status: statusFilter,
    }));
  }, [selectedTools, statusFilter]);

  const hasActiveFilters = searchQuery || statusFilter !== 'all' || !selectedTools.has('all');

  return (
    <div
      className="h-full flex flex-col bg-white dark:bg-gray-900 border-l border-gray-200 dark:border-gray-700"
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <MixerHorizontalIcon className="w-4 h-4 text-gray-500" />
          <h3 className="font-semibold text-gray-900 dark:text-white">
            {t('sidebar.title')}
          </h3>
        </div>
        <div className="flex items-center gap-1">
          {/* Timeline toggle */}
          <button
            onClick={() => setShowTimeline(!showTimeline)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              showTimeline
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800'
            )}
            title="Toggle timeline view"
          >
            <ClockIcon className="w-4 h-4" />
          </button>

          {/* Stats toggle */}
          <button
            onClick={() => setShowStats(!showStats)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              showStats
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800'
            )}
            title="Toggle statistics"
          >
            <BarChartIcon className="w-4 h-4" />
          </button>

          {/* Export */}
          <button
            onClick={handleExport}
            disabled={filteredToolCalls.length === 0}
            className="p-1.5 rounded text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50 transition-colors"
            title="Export filtered results"
          >
            <DownloadIcon className="w-4 h-4" />
          </button>

          {/* Close */}
          {onClose && (
            <button
              onClick={onClose}
              className="p-1.5 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-500"
            >
              <Cross2Icon className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Search */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <SearchInput
          value={searchQuery}
          onChange={setSearchQuery}
          onClear={() => setSearchQuery('')}
        />
      </div>

      {/* Quick stats */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <div className="grid grid-cols-4 gap-2 text-center">
          <StatItem
            value={stats.byStatus.completed || 0}
            label={t('sidebar.stats.done')}
            color="text-green-600 dark:text-green-400"
          />
          <StatItem
            value={stats.byStatus.failed || 0}
            label={t('sidebar.stats.failed')}
            color="text-red-600 dark:text-red-400"
          />
          <StatItem
            value={stats.byStatus.executing || 0}
            label={t('sidebar.stats.running')}
            color="text-blue-600 dark:text-blue-400"
          />
          <StatItem
            value={stats.byStatus.pending || 0}
            label="Pending"
            color="text-gray-600 dark:text-gray-400"
          />
        </div>
      </div>

      {/* Filters */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 space-y-3">
        {/* Status filter */}
        <div className="flex flex-wrap gap-1.5">
          {(['all', 'pending', 'executing', 'completed', 'failed'] as StatusFilter[]).map((status) => (
            <button
              key={status}
              onClick={() => setStatusFilter(status)}
              className={clsx(
                'px-2 py-1 rounded-full text-xs font-medium transition-colors',
                statusFilter === status
                  ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
              )}
            >
              {status === 'all' ? 'All Status' : status.charAt(0).toUpperCase() + status.slice(1)}
            </button>
          ))}
        </div>

        {/* Tool type filter (multi-select) */}
        <div className="flex flex-wrap gap-1.5">
          <button
            onClick={() => handleToolToggle('all')}
            className={clsx(
              'px-2 py-1 rounded-full text-xs font-medium transition-colors',
              selectedTools.has('all')
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
            )}
          >
            All Tools
          </button>
          {toolTypes.map((type) => {
            const count = stats.byTool[type]?.count || 0;
            if (count === 0 && !selectedTools.has(type)) return null;

            const config = getToolConfig(type);

            return (
              <button
                key={type}
                onClick={() => handleToolToggle(type)}
                className={clsx(
                  'flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium transition-colors',
                  selectedTools.has(type)
                    ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300'
                    : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
                )}
              >
                <config.Icon className="w-3 h-3" />
                <span>{type}</span>
                <span className="text-gray-400 dark:text-gray-500">({count})</span>
              </button>
            );
          })}
        </div>

        {/* Clear filters */}
        {hasActiveFilters && (
          <button
            onClick={handleClearFilters}
            className="flex items-center gap-1 text-xs text-primary-600 dark:text-primary-400 hover:underline"
          >
            <Cross2Icon className="w-3 h-3" />
            Clear all filters
          </button>
        )}
      </div>

      {/* Timeline view */}
      {showTimeline && toolCallHistory.length > 0 && (
        <div className="border-b border-gray-200 dark:border-gray-700">
          <ExecutionTimeline
            toolCalls={filteredToolCalls}
            onToolClick={onToolClick}
            height={200}
          />
        </div>
      )}

      {/* Statistics panel */}
      {showStats && (
        <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
          <StatisticsPanel stats={stats} />
        </div>
      )}

      {/* Tool Call List */}
      <div className="flex-1 overflow-auto">
        {filteredToolCalls.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4">
            <MagnifyingGlassIcon className="w-8 h-8 mb-2 opacity-50" />
            <p className="text-sm">
              {hasActiveFilters ? 'No matching tool calls' : t('sidebar.empty')}
            </p>
            {hasActiveFilters && (
              <button
                onClick={handleClearFilters}
                className="mt-2 text-xs text-primary-600 dark:text-primary-400 hover:underline"
              >
                Clear filters
              </button>
            )}
          </div>
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {filteredToolCalls.map((toolCall, index) => (
              <ToolCallItem
                key={toolCall.id}
                toolCall={toolCall}
                index={index}
                isFocused={index === focusedIndex}
                searchQuery={searchQuery}
                onClick={() => onToolClick?.(toolCall.id)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Footer with bulk actions */}
      <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <div className="flex items-center justify-between text-xs text-gray-500">
          <span>
            {filteredToolCalls.length} of {toolCallHistory.length} tool calls
          </span>
          <button
            onClick={() => {
              if (confirm('Clear all tool history?')) {
                clearConversation();
              }
            }}
            disabled={toolCallHistory.length === 0}
            className="flex items-center gap-1 text-red-500 hover:text-red-700 disabled:opacity-50"
          >
            <TrashIcon className="w-3 h-3" />
            Clear history
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// StatItem Component
// ============================================================================

interface StatItemProps {
  value: number;
  label: string;
  color: string;
}

function StatItem({ value, label, color }: StatItemProps) {
  return (
    <div>
      <div className={clsx('text-lg font-semibold', color)}>{value}</div>
      <div className="text-xs text-gray-500 dark:text-gray-400">{label}</div>
    </div>
  );
}

// ============================================================================
// ToolCallItem Component
// ============================================================================

interface ToolCallItemProps {
  toolCall: ToolCall;
  index: number;
  isFocused: boolean;
  searchQuery: string;
  onClick?: () => void;
}

function ToolCallItem({ toolCall, index, isFocused, searchQuery, onClick }: ToolCallItemProps) {
  const config = getToolConfig(toolCall.name);
  const statusConfig = getStatusConfig(toolCall.status);

  // Highlight search matches
  const highlightText = (text: string) => {
    if (!searchQuery.trim()) return text;

    const parts = text.split(new RegExp(`(${searchQuery})`, 'gi'));
    return parts.map((part, i) =>
      part.toLowerCase() === searchQuery.toLowerCase() ? (
        <mark key={i} className="bg-yellow-200 dark:bg-yellow-800 text-yellow-900 dark:text-yellow-100 rounded px-0.5">
          {part}
        </mark>
      ) : (
        part
      )
    );
  };

  return (
    <button
      onClick={onClick}
      className={clsx(
        'w-full flex items-center gap-3 px-4 py-2.5',
        'hover:bg-gray-50 dark:hover:bg-gray-800/50',
        'transition-colors text-left',
        isFocused && 'bg-primary-50 dark:bg-primary-900/20'
      )}
    >
      {/* Index */}
      <span className="w-6 h-6 flex items-center justify-center rounded-full bg-gray-100 dark:bg-gray-800 text-xs text-gray-500">
        {index + 1}
      </span>

      {/* Tool icon */}
      <span className={clsx('p-1.5 rounded', config.iconBg)}>
        <config.Icon className={clsx('w-3.5 h-3.5', config.iconColor)} />
      </span>

      {/* Tool info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="text-sm font-medium text-gray-900 dark:text-white">
            {highlightText(toolCall.name)}
          </span>
          <statusConfig.Icon
            className={clsx(
              'w-3.5 h-3.5',
              statusConfig.textColor,
              toolCall.status === 'executing' && 'animate-spin'
            )}
          />
        </div>
        <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
          {highlightText(getToolSummary(toolCall))}
        </div>
      </div>

      {/* Duration */}
      {toolCall.duration && (
        <span className="text-xs text-gray-400 dark:text-gray-500">
          {formatDuration(toolCall.duration)}
        </span>
      )}
    </button>
  );
}

// ============================================================================
// Helper Functions
// ============================================================================

function getToolConfig(name: ToolType | string) {
  switch (name) {
    case 'Read':
      return {
        Icon: FileTextIcon,
        iconBg: 'bg-blue-100 dark:bg-blue-900/50',
        iconColor: 'text-blue-600 dark:text-blue-400',
        barColor: 'bg-blue-500',
      };
    case 'Write':
      return {
        Icon: FileTextIcon,
        iconBg: 'bg-green-100 dark:bg-green-900/50',
        iconColor: 'text-green-600 dark:text-green-400',
        barColor: 'bg-green-500',
      };
    case 'Edit':
      return {
        Icon: Pencil1Icon,
        iconBg: 'bg-yellow-100 dark:bg-yellow-900/50',
        iconColor: 'text-yellow-600 dark:text-yellow-400',
        barColor: 'bg-yellow-500',
      };
    case 'Bash':
      return {
        Icon: CodeIcon,
        iconBg: 'bg-purple-100 dark:bg-purple-900/50',
        iconColor: 'text-purple-600 dark:text-purple-400',
        barColor: 'bg-purple-500',
      };
    case 'Glob':
      return {
        Icon: MagnifyingGlassIcon,
        iconBg: 'bg-orange-100 dark:bg-orange-900/50',
        iconColor: 'text-orange-600 dark:text-orange-400',
        barColor: 'bg-orange-500',
      };
    case 'Grep':
      return {
        Icon: MagnifyingGlassIcon,
        iconBg: 'bg-pink-100 dark:bg-pink-900/50',
        iconColor: 'text-pink-600 dark:text-pink-400',
        barColor: 'bg-pink-500',
      };
    case 'WebFetch':
    case 'WebSearch':
      return {
        Icon: GlobeIcon,
        iconBg: 'bg-cyan-100 dark:bg-cyan-900/50',
        iconColor: 'text-cyan-600 dark:text-cyan-400',
        barColor: 'bg-cyan-500',
      };
    default:
      return {
        Icon: FileIcon,
        iconBg: 'bg-gray-100 dark:bg-gray-800',
        iconColor: 'text-gray-600 dark:text-gray-400',
        barColor: 'bg-gray-500',
      };
  }
}

function getStatusConfig(status: string) {
  switch (status) {
    case 'pending':
      return {
        Icon: ReloadIcon,
        textColor: 'text-gray-500',
      };
    case 'executing':
      return {
        Icon: ReloadIcon,
        textColor: 'text-blue-500',
      };
    case 'completed':
      return {
        Icon: CheckCircledIcon,
        textColor: 'text-green-500',
      };
    case 'failed':
      return {
        Icon: CrossCircledIcon,
        textColor: 'text-red-500',
      };
    default:
      return {
        Icon: ReloadIcon,
        textColor: 'text-gray-500',
      };
  }
}

function getToolSummary(toolCall: ToolCall): string {
  if (toolCall.parameters.file_path) {
    const path = toolCall.parameters.file_path;
    const parts = path.split(/[/\\]/);
    return parts.pop() || path;
  }
  if (toolCall.parameters.command) {
    const cmd = toolCall.parameters.command;
    return cmd.length > 30 ? cmd.slice(0, 30) + '...' : cmd;
  }
  if (toolCall.parameters.pattern) {
    return toolCall.parameters.pattern;
  }
  return '';
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

export default ToolHistorySidebar;
