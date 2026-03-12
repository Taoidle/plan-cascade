import { useCallback, useMemo, useState } from 'react';
import { clsx } from 'clsx';
import {
  CheckCircledIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  CodeIcon,
  CrossCircledIcon,
  LightningBoltIcon,
  UpdateIcon,
} from '@radix-ui/react-icons';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import type { QualityGateOutcome, QualityGateStatus } from '../../types/workflowQuality';

interface QualityGateBadgeProps {
  storyId?: string;
  className?: string;
  compact?: boolean;
}

type BadgeResult = QualityGateOutcome & { scopeId: string };

interface SingleBadgeProps {
  result: BadgeResult;
  compact?: boolean;
}

const GATE_ICONS: Record<string, React.ReactNode> = {
  typecheck: <CodeIcon className="w-3.5 h-3.5" />,
  test: <CheckCircledIcon className="w-3.5 h-3.5" />,
  lint: <LightningBoltIcon className="w-3.5 h-3.5" />,
};

const STATUS_CONFIG: Record<
  QualityGateStatus,
  {
    bg: string;
    text: string;
    border: string;
    icon: React.ReactNode;
  }
> = {
  pending: {
    bg: 'bg-gray-100 dark:bg-gray-800',
    text: 'text-gray-600 dark:text-gray-400',
    border: 'border-gray-300 dark:border-gray-600',
    icon: <ClockIcon className="w-3.5 h-3.5" />,
  },
  running: {
    bg: 'bg-warning-50 dark:bg-warning-950',
    text: 'text-warning-700 dark:text-warning-300',
    border: 'border-warning-300 dark:border-warning-700',
    icon: <UpdateIcon className="w-3.5 h-3.5 animate-spin" />,
  },
  passed: {
    bg: 'bg-success-50 dark:bg-success-950',
    text: 'text-success-700 dark:text-success-300',
    border: 'border-success-300 dark:border-success-700',
    icon: <CheckCircledIcon className="w-3.5 h-3.5" />,
  },
  failed: {
    bg: 'bg-error-50 dark:bg-error-950',
    text: 'text-error-700 dark:text-error-300',
    border: 'border-error-300 dark:border-error-700',
    icon: <CrossCircledIcon className="w-3.5 h-3.5" />,
  },
  warning: {
    bg: 'bg-warning-50 dark:bg-warning-950',
    text: 'text-warning-700 dark:text-warning-300',
    border: 'border-warning-300 dark:border-warning-700',
    icon: <LightningBoltIcon className="w-3.5 h-3.5" />,
  },
  skipped: {
    bg: 'bg-gray-100 dark:bg-gray-800',
    text: 'text-gray-500 dark:text-gray-400',
    border: 'border-gray-200 dark:border-gray-700',
    icon: <ClockIcon className="w-3.5 h-3.5" />,
  },
};

function getGateIcon(gateId: string): React.ReactNode {
  return GATE_ICONS[gateId.toLowerCase()] || <CodeIcon className="w-3.5 h-3.5" />;
}

function SingleBadge({ result, compact = false }: SingleBadgeProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const config = STATUS_CONFIG[result.status];
  const isExpandable = !!result.message && !compact;

  const handleClick = useCallback(() => {
    if (isExpandable) {
      setIsExpanded((previous) => !previous);
    }
  }, [isExpandable]);

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
          isExpandable ? 'cursor-pointer hover:shadow-sm' : 'cursor-default',
        )}
      >
        {getGateIcon(result.gateId)}
        <span className="font-medium">{result.gateName}</span>
        {config.icon}
        {typeof result.durationMs === 'number' && !compact && (
          <span className="opacity-70 text-2xs">
            {result.durationMs < 1000 ? `${result.durationMs}ms` : `${(result.durationMs / 1000).toFixed(1)}s`}
          </span>
        )}
        {isExpandable &&
          (isExpanded ? (
            <ChevronDownIcon className="w-3 h-3 ml-0.5" />
          ) : (
            <ChevronRightIcon className="w-3 h-3 ml-0.5" />
          ))}
      </button>
      {isExpanded && result.message && (
        <div className="mt-2 rounded-lg border border-gray-200 bg-gray-50 p-3 text-xs text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-300">
          {result.message}
        </div>
      )}
    </div>
  );
}

export function QualityGateBadge({ storyId, className, compact = false }: QualityGateBadgeProps) {
  const session = useWorkflowKernelStore((state) => state.session);

  const groupedResults = useMemo(() => {
    const runs = [
      ...(session?.modeSnapshots.chat?.quality?.runs ?? []),
      ...(session?.modeSnapshots.plan?.quality?.runs ?? []),
      ...(session?.modeSnapshots.task?.quality?.runs ?? []),
      ...(session?.modeSnapshots.debug?.quality?.runs ?? []),
    ];

    const filteredRuns = storyId ? runs.filter((run) => run.scopeId === storyId) : runs;
    const grouped: Record<string, BadgeResult[]> = {};
    for (const run of filteredRuns) {
      const key = run.scopeId ?? run.runId;
      grouped[key] = run.outcomes.map((outcome) => ({ ...outcome, scopeId: key }));
    }
    return grouped;
  }, [session, storyId]);

  const entries = Object.entries(groupedResults);
  if (entries.length === 0) return null;

  if (storyId) {
    return (
      <div className={clsx('flex flex-wrap gap-2', className)}>
        {(groupedResults[storyId] ?? []).map((result) => (
          <SingleBadge key={`${result.gateId}-${result.scopeId}`} result={result} compact={compact} />
        ))}
      </div>
    );
  }

  return (
    <div className={clsx('space-y-3', className)}>
      {entries.map(([scopeId, results]) => (
        <div key={scopeId}>
          <div className="mb-1.5 text-xs font-medium text-gray-500 dark:text-gray-400">{scopeId}</div>
          <div className="flex flex-wrap gap-2">
            {results.map((result) => (
              <SingleBadge key={`${result.gateId}-${result.scopeId}`} result={result} compact={compact} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

export default QualityGateBadge;
