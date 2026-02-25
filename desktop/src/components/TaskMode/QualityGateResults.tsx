/**
 * Quality Gate Results Component
 *
 * Displays per-story quality gate pipeline results with phase breakdown,
 * code review dimension scores, and pass/fail badges.
 *
 * Story 007: Frontend Task Mode Store and UI Components
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { CheckCircledIcon, CrossCircledIcon, ClockIcon, UpdateIcon, MinusCircledIcon } from '@radix-ui/react-icons';
import type { StoryQualityGateResults, GateResult, GatePhase, GateStatus, DimensionScore } from '../../store/taskMode';

// ============================================================================
// Helpers
// ============================================================================

const statusIcons: Record<GateStatus, React.ReactNode> = {
  pending: <ClockIcon className="w-3.5 h-3.5 text-gray-400" />,
  running: <UpdateIcon className="w-3.5 h-3.5 text-blue-500 animate-spin" />,
  passed: <CheckCircledIcon className="w-3.5 h-3.5 text-green-500" />,
  failed: <CrossCircledIcon className="w-3.5 h-3.5 text-red-500" />,
  skipped: <MinusCircledIcon className="w-3.5 h-3.5 text-gray-400" />,
};

const statusColors: Record<GateStatus, string> = {
  pending: 'text-gray-500 dark:text-gray-400',
  running: 'text-blue-600 dark:text-blue-400',
  passed: 'text-green-600 dark:text-green-400',
  failed: 'text-red-600 dark:text-red-400',
  skipped: 'text-gray-400 dark:text-gray-500',
};

const phaseOrder: GatePhase[] = ['pre_validation', 'validation', 'post_validation'];

function groupByPhase(gates: GateResult[]): Record<GatePhase, GateResult[]> {
  const grouped: Record<GatePhase, GateResult[]> = {
    pre_validation: [],
    validation: [],
    post_validation: [],
  };
  for (const gate of gates) {
    if (grouped[gate.phase]) {
      grouped[gate.phase].push(gate);
    }
  }
  return grouped;
}

// ============================================================================
// Sub-Components
// ============================================================================

interface GateBadgeProps {
  gate: GateResult;
}

function GateBadge({ gate }: GateBadgeProps) {
  return (
    <div
      className={clsx(
        'flex items-center gap-1.5 px-2 py-1 rounded-md text-xs',
        'border',
        gate.status === 'passed' && 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20',
        gate.status === 'failed' && 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20',
        gate.status === 'running' && 'border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20',
        gate.status === 'pending' && 'border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800',
        gate.status === 'skipped' && 'border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800',
      )}
      title={gate.message}
    >
      {statusIcons[gate.status]}
      <span className={clsx('font-medium', statusColors[gate.status])}>{gate.gateName}</span>
      {gate.duration !== undefined && gate.duration > 0 && (
        <span className="text-gray-400 dark:text-gray-500">{(gate.duration / 1000).toFixed(1)}s</span>
      )}
    </div>
  );
}

interface DimensionScoreBarProps {
  dimension: DimensionScore;
}

function DimensionScoreBar({ dimension }: DimensionScoreBarProps) {
  const percent = dimension.maxScore > 0 ? Math.round((dimension.score / dimension.maxScore) * 100) : 0;

  const color = percent >= 80 ? 'bg-green-500' : percent >= 60 ? 'bg-yellow-500' : 'bg-red-500';

  return (
    <div className="space-y-0.5">
      <div className="flex items-center justify-between text-xs">
        <span className="text-gray-600 dark:text-gray-400">{dimension.dimension}</span>
        <span className="text-gray-500 dark:text-gray-400 font-mono">
          {dimension.score}/{dimension.maxScore}
        </span>
      </div>
      <div className="h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div className={clsx('h-full rounded-full transition-all', color)} style={{ width: `${percent}%` }} />
      </div>
      {dimension.feedback && <p className="text-xs text-gray-400 dark:text-gray-500 truncate">{dimension.feedback}</p>}
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

interface QualityGateResultsProps {
  /** Quality gate results for a single story */
  results: StoryQualityGateResults;
  /** Whether to show expanded view with code review scores */
  expanded?: boolean;
}

export function QualityGateResults({ results, expanded = false }: QualityGateResultsProps) {
  const { t } = useTranslation('taskMode');
  const phaseGroups = groupByPhase(results.gates);

  return (
    <div className="space-y-3" data-testid="quality-gate-results">
      {/* Overall status badge */}
      <div className="flex items-center gap-2">
        {statusIcons[results.overallStatus]}
        <span className={clsx('text-sm font-medium', statusColors[results.overallStatus])}>
          {t('qualityGates.title')}
        </span>
        {results.totalScore !== undefined && (
          <span className="text-xs text-gray-500 dark:text-gray-400 ml-auto font-mono">
            {t('qualityGates.codeReview.totalScore', { score: results.totalScore })}
          </span>
        )}
      </div>

      {/* Phase breakdown */}
      {phaseOrder.map((phase) => {
        const gates = phaseGroups[phase];
        if (gates.length === 0) return null;

        const phaseKey =
          phase === 'pre_validation' ? 'preValidation' : phase === 'post_validation' ? 'postValidation' : 'validation';

        return (
          <div key={phase} className="space-y-1.5">
            <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
              {t(`qualityGates.phase.${phaseKey}`)}
            </span>
            <div className="flex flex-wrap gap-1.5">
              {gates.map((gate) => (
                <GateBadge key={gate.gateId} gate={gate} />
              ))}
            </div>
          </div>
        );
      })}

      {/* Code review dimension scores (expanded view) */}
      {expanded && results.codeReviewScores && results.codeReviewScores.length > 0 && (
        <div className="space-y-2 pt-2 border-t border-gray-200 dark:border-gray-700">
          <span className="text-xs font-medium text-gray-600 dark:text-gray-300">
            {t('qualityGates.codeReview.title')}
          </span>
          <div className="space-y-2">
            {results.codeReviewScores.map((dim) => (
              <DimensionScoreBar key={dim.dimension} dimension={dim} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Multi-Story Summary
// ============================================================================

interface QualityGatesSummaryProps {
  /** All quality gate results keyed by story ID */
  results: Record<string, StoryQualityGateResults>;
}

export function QualityGatesSummary({ results }: QualityGatesSummaryProps) {
  const { t } = useTranslation('taskMode');
  const storyIds = Object.keys(results);

  if (storyIds.length === 0) {
    return <p className="text-sm text-gray-500 dark:text-gray-400 italic">{t('qualityGates.noResults')}</p>;
  }

  return (
    <div className="space-y-4" data-testid="quality-gates-summary">
      {storyIds.map((storyId) => (
        <div
          key={storyId}
          className={clsx(
            'p-3 rounded-lg',
            'border border-gray-200 dark:border-gray-700',
            'bg-gray-50 dark:bg-gray-800/50',
          )}
        >
          <div className="flex items-center gap-2 mb-2">
            {statusIcons[results[storyId].overallStatus]}
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">{storyId}</span>
          </div>
          <QualityGateResults results={results[storyId]} expanded />
        </div>
      ))}
    </div>
  );
}

export default QualityGateResults;
