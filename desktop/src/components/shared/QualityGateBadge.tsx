/**
 * QualityGateBadge Component
 *
 * Pass/fail badge for each quality gate type (TypeCheck, Test, Lint).
 * Expandable detail panel on click showing full output for failures.
 * Animated enter transition with color coding.
 *
 * Story 008: Real-time Execution Feedback
 */

import { useState, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import {
  CheckCircledIcon,
  CrossCircledIcon,
  UpdateIcon,
  ClockIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CodeIcon,
  CheckIcon,
  LightningBoltIcon,
} from '@radix-ui/react-icons';
import { useExecutionStore, QualityGateResult, QualityGateStatus } from '../../store/execution';

// ============================================================================
// Types
// ============================================================================

interface QualityGateBadgeProps {
  /** Filter results by story ID. If not provided, shows all results. */
  storyId?: string;
  /** Additional class names */
  className?: string;
  /** Compact display (inline badges only, no expandable detail) */
  compact?: boolean;
}

interface SingleBadgeProps {
  result: QualityGateResult;
  compact?: boolean;
}

// ============================================================================
// Gate Icons
// ============================================================================

const GATE_ICONS: Record<string, React.ReactNode> = {
  typecheck: <CodeIcon className="w-3.5 h-3.5" />,
  type_check: <CodeIcon className="w-3.5 h-3.5" />,
  test: <CheckIcon className="w-3.5 h-3.5" />,
  lint: <LightningBoltIcon className="w-3.5 h-3.5" />,
};

function getGateIcon(gateId: string): React.ReactNode {
  const normalized = gateId.toLowerCase().replace(/\s+/g, '_');
  return GATE_ICONS[normalized] || <ClockIcon className="w-3.5 h-3.5" />;
}

// ============================================================================
// Status Colors and Icons
// ============================================================================

const STATUS_CONFIG: Record<
  QualityGateStatus,
  {
    bg: string;
    text: string;
    border: string;
    icon: React.ReactNode;
    label: string;
  }
> = {
  pending: {
    bg: 'bg-gray-100 dark:bg-gray-800',
    text: 'text-gray-600 dark:text-gray-400',
    border: 'border-gray-300 dark:border-gray-600',
    icon: <ClockIcon className="w-3.5 h-3.5" />,
    label: 'Pending',
  },
  running: {
    bg: 'bg-warning-50 dark:bg-warning-950',
    text: 'text-warning-700 dark:text-warning-300',
    border: 'border-warning-300 dark:border-warning-700',
    icon: <UpdateIcon className="w-3.5 h-3.5 animate-spin" />,
    label: 'Running',
  },
  passed: {
    bg: 'bg-success-50 dark:bg-success-950',
    text: 'text-success-700 dark:text-success-300',
    border: 'border-success-300 dark:border-success-700',
    icon: <CheckCircledIcon className="w-3.5 h-3.5" />,
    label: 'Passed',
  },
  failed: {
    bg: 'bg-error-50 dark:bg-error-950',
    text: 'text-error-700 dark:text-error-300',
    border: 'border-error-300 dark:border-error-700',
    icon: <CrossCircledIcon className="w-3.5 h-3.5" />,
    label: 'Failed',
  },
};

// ============================================================================
// SingleBadge Component
// ============================================================================

function SingleBadge({ result, compact = false }: SingleBadgeProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const config = STATUS_CONFIG[result.status];
  const hasOutput = !!result.output;
  const isExpandable = hasOutput && !compact;

  const handleClick = useCallback(() => {
    if (isExpandable) {
      setIsExpanded((prev) => !prev);
    }
  }, [isExpandable]);

  const formatDuration = (ms: number): string => {
    if (ms < 1000) return `${ms}ms`;
    const seconds = (ms / 1000).toFixed(1);
    return `${seconds}s`;
  };

  return (
    <div className={clsx('animate-[fadeIn_0.3s_ease-in-out]', !compact && 'transition-all duration-200')}>
      <button
        onClick={handleClick}
        disabled={!isExpandable}
        className={clsx(
          'inline-flex items-center gap-1.5 rounded-full border transition-all duration-200',
          config.bg,
          config.text,
          config.border,
          compact ? 'px-2 py-0.5 text-2xs' : 'px-3 py-1 text-xs',
          isExpandable && 'cursor-pointer hover:shadow-sm',
          !isExpandable && 'cursor-default',
          result.status === 'passed' && 'animate-[scaleIn_0.3s_ease-out]',
          result.status === 'failed' && 'animate-[shakeX_0.4s_ease-in-out]',
        )}
      >
        {getGateIcon(result.gateId)}
        <span className="font-medium">{result.gateName}</span>
        {config.icon}
        {result.duration !== undefined && !compact && (
          <span className="opacity-70 text-2xs">{formatDuration(result.duration)}</span>
        )}
        {isExpandable &&
          (isExpanded ? (
            <ChevronDownIcon className="w-3 h-3 ml-0.5" />
          ) : (
            <ChevronRightIcon className="w-3 h-3 ml-0.5" />
          ))}
      </button>

      {/* Expandable detail panel */}
      {isExpanded && hasOutput && (
        <div
          className={clsx(
            'mt-2 rounded-lg overflow-hidden border',
            'bg-gray-50 dark:bg-gray-900',
            'border-gray-200 dark:border-gray-700',
            'animate-[slideDown_0.2s_ease-out]',
          )}
        >
          <div
            className={clsx(
              'flex items-center justify-between px-3 py-1.5',
              'bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700',
            )}
          >
            <span className={clsx('text-2xs font-medium', config.text)}>{result.gateName} Output</span>
            {result.duration !== undefined && (
              <span className="text-2xs text-gray-500">{formatDuration(result.duration)}</span>
            )}
          </div>
          <pre
            className={clsx(
              'p-3 text-xs font-mono',
              'text-gray-700 dark:text-gray-300',
              'whitespace-pre-wrap break-all',
              'max-h-64 overflow-y-auto',
            )}
          >
            {result.output}
          </pre>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// QualityGateBadge Component
// ============================================================================

export function QualityGateBadge({ storyId, className, compact = false }: QualityGateBadgeProps) {
  const { qualityGateResults } = useExecutionStore();

  const filteredResults = useMemo(() => {
    if (storyId) {
      return qualityGateResults.filter((r) => r.storyId === storyId);
    }
    return qualityGateResults;
  }, [qualityGateResults, storyId]);

  // Group results by story if no storyId filter
  const groupedResults = useMemo(() => {
    if (storyId) {
      return { [storyId]: filteredResults };
    }
    const groups: Record<string, QualityGateResult[]> = {};
    for (const result of filteredResults) {
      if (!groups[result.storyId]) {
        groups[result.storyId] = [];
      }
      groups[result.storyId].push(result);
    }
    return groups;
  }, [filteredResults, storyId]);

  if (filteredResults.length === 0) return null;

  // If filtered by story, render inline badges
  if (storyId) {
    return (
      <div className={clsx('flex flex-wrap gap-2', className)}>
        {filteredResults.map((result) => (
          <SingleBadge key={`${result.gateId}-${result.storyId}`} result={result} compact={compact} />
        ))}
      </div>
    );
  }

  // Otherwise render grouped by story
  return (
    <div className={clsx('space-y-3', className)}>
      {Object.entries(groupedResults).map(([sid, results]) => (
        <div key={sid}>
          <div className="text-xs text-gray-500 dark:text-gray-400 mb-1.5 font-medium">{sid}</div>
          <div className="flex flex-wrap gap-2">
            {results.map((result) => (
              <SingleBadge key={`${result.gateId}-${result.storyId}`} result={result} compact={compact} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

export default QualityGateBadge;
