/**
 * ToolHistorySidebar Component
 *
 * Shows the history of all tool calls in the current conversation.
 * Allows filtering by tool type and clicking to jump to specific tool calls.
 */

import { useMemo } from 'react';
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
} from '@radix-ui/react-icons';
import { useClaudeCodeStore, ToolType, ToolCall } from '../../store/claudeCode';

// ============================================================================
// ToolHistorySidebar Component
// ============================================================================

interface ToolHistorySidebarProps {
  onToolClick?: (toolCallId: string) => void;
  onClose?: () => void;
}

export function ToolHistorySidebar({ onToolClick, onClose }: ToolHistorySidebarProps) {
  const { t } = useTranslation('claudeCode');
  const { toolCallHistory, toolFilter, setToolFilter } = useClaudeCodeStore();

  // Calculate statistics
  const stats = useMemo(() => {
    const counts: Record<string, number> = { all: 0 };
    const statusCounts = { completed: 0, failed: 0, executing: 0, pending: 0 };

    toolCallHistory.forEach((tc) => {
      counts.all++;
      counts[tc.name] = (counts[tc.name] || 0) + 1;
      statusCounts[tc.status]++;
    });

    return { counts, statusCounts };
  }, [toolCallHistory]);

  // Filter tool calls
  const filteredToolCalls = useMemo(() => {
    if (toolFilter === 'all') return toolCallHistory;
    return toolCallHistory.filter((tc) => tc.name === toolFilter);
  }, [toolCallHistory, toolFilter]);

  const toolTypes: (ToolType | 'all')[] = [
    'all',
    'Read',
    'Write',
    'Edit',
    'Bash',
    'Glob',
    'Grep',
    'WebFetch',
    'WebSearch',
  ];

  return (
    <div className="h-full flex flex-col bg-white dark:bg-gray-900 border-l border-gray-200 dark:border-gray-700">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <MixerHorizontalIcon className="w-4 h-4 text-gray-500" />
          <h3 className="font-semibold text-gray-900 dark:text-white">
            {t('sidebar.title')}
          </h3>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-500"
          >
            <Cross2Icon className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Statistics Summary */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <div className="grid grid-cols-4 gap-2 text-center">
          <StatItem
            value={stats.counts.all}
            label={t('sidebar.stats.total')}
            color="text-gray-600 dark:text-gray-400"
          />
          <StatItem
            value={stats.statusCounts.completed}
            label={t('sidebar.stats.done')}
            color="text-green-600 dark:text-green-400"
          />
          <StatItem
            value={stats.statusCounts.failed}
            label={t('sidebar.stats.failed')}
            color="text-red-600 dark:text-red-400"
          />
          <StatItem
            value={stats.statusCounts.executing}
            label={t('sidebar.stats.running')}
            color="text-blue-600 dark:text-blue-400"
          />
        </div>
      </div>

      {/* Filter Pills */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex flex-wrap gap-1.5">
          {toolTypes.map((type) => {
            const count = type === 'all' ? stats.counts.all : (stats.counts[type] || 0);
            if (type !== 'all' && count === 0) return null;

            return (
              <FilterPill
                key={type}
                type={type}
                count={count}
                isActive={toolFilter === type}
                onClick={() => setToolFilter(type)}
              />
            );
          })}
        </div>
      </div>

      {/* Tool Call List */}
      <div className="flex-1 overflow-auto">
        {filteredToolCalls.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4">
            <MagnifyingGlassIcon className="w-8 h-8 mb-2 opacity-50" />
            <p className="text-sm">{t('sidebar.empty')}</p>
          </div>
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {filteredToolCalls.map((toolCall, index) => (
              <ToolCallItem
                key={toolCall.id}
                toolCall={toolCall}
                index={index}
                onClick={() => onToolClick?.(toolCall.id)}
              />
            ))}
          </div>
        )}
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
// FilterPill Component
// ============================================================================

interface FilterPillProps {
  type: ToolType | 'all';
  count: number;
  isActive: boolean;
  onClick: () => void;
}

function FilterPill({ type, count, isActive, onClick }: FilterPillProps) {
  const { t } = useTranslation('claudeCode');
  const config = getToolConfig(type);

  return (
    <button
      onClick={onClick}
      className={clsx(
        'flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium transition-colors',
        isActive
          ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300'
          : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
      )}
    >
      {type !== 'all' && <config.Icon className="w-3 h-3" />}
      <span>{type === 'all' ? t('sidebar.filter.all') : type}</span>
      <span className="text-gray-400 dark:text-gray-500">({count})</span>
    </button>
  );
}

// ============================================================================
// ToolCallItem Component
// ============================================================================

interface ToolCallItemProps {
  toolCall: ToolCall;
  index: number;
  onClick?: () => void;
}

function ToolCallItem({ toolCall, index, onClick }: ToolCallItemProps) {
  const config = getToolConfig(toolCall.name);
  const statusConfig = getStatusConfig(toolCall.status);

  return (
    <button
      onClick={onClick}
      className={clsx(
        'w-full flex items-center gap-3 px-4 py-2.5',
        'hover:bg-gray-50 dark:hover:bg-gray-800/50',
        'transition-colors text-left'
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
            {toolCall.name}
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
          {getToolSummary(toolCall)}
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

function getToolConfig(name: ToolType | 'all') {
  switch (name) {
    case 'Read':
      return {
        Icon: FileTextIcon,
        iconBg: 'bg-blue-100 dark:bg-blue-900/50',
        iconColor: 'text-blue-600 dark:text-blue-400',
      };
    case 'Write':
      return {
        Icon: FileTextIcon,
        iconBg: 'bg-green-100 dark:bg-green-900/50',
        iconColor: 'text-green-600 dark:text-green-400',
      };
    case 'Edit':
      return {
        Icon: Pencil1Icon,
        iconBg: 'bg-yellow-100 dark:bg-yellow-900/50',
        iconColor: 'text-yellow-600 dark:text-yellow-400',
      };
    case 'Bash':
      return {
        Icon: CodeIcon,
        iconBg: 'bg-purple-100 dark:bg-purple-900/50',
        iconColor: 'text-purple-600 dark:text-purple-400',
      };
    case 'Glob':
      return {
        Icon: MagnifyingGlassIcon,
        iconBg: 'bg-orange-100 dark:bg-orange-900/50',
        iconColor: 'text-orange-600 dark:text-orange-400',
      };
    case 'Grep':
      return {
        Icon: MagnifyingGlassIcon,
        iconBg: 'bg-pink-100 dark:bg-pink-900/50',
        iconColor: 'text-pink-600 dark:text-pink-400',
      };
    case 'WebFetch':
    case 'WebSearch':
      return {
        Icon: GlobeIcon,
        iconBg: 'bg-cyan-100 dark:bg-cyan-900/50',
        iconColor: 'text-cyan-600 dark:text-cyan-400',
      };
    default:
      return {
        Icon: FileIcon,
        iconBg: 'bg-gray-100 dark:bg-gray-800',
        iconColor: 'text-gray-600 dark:text-gray-400',
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
