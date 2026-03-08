/**
 * StrategyCard
 *
 * Displays strategy analysis results with dimension bars and recommendations.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { StrategyCardData } from '../../../types/workflowCard';

export function StrategyCard({ data }: { data: StrategyCardData }) {
  const { t } = useTranslation('simpleMode');
  const confidencePct = Math.round(data.confidence * 100);
  const localizedStrategy = localizeStrategyValue(t, data.strategy);
  const localizedRisk = localizeStrategyRisk(t, data.riskLevel);
  const localizedParallel = localizeStrategyParallel(t, data.parallelizationBenefit);

  return (
    <div className="rounded-lg border border-indigo-200 dark:border-indigo-800 bg-indigo-50 dark:bg-indigo-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-indigo-100/50 dark:bg-indigo-900/30 border-b border-indigo-200 dark:border-indigo-800">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-indigo-700 dark:text-indigo-300 uppercase tracking-wide">
            {t('workflow.strategy.title')}
          </span>
          <div className="flex items-center gap-2">
            {data.model && (
              <span className="text-2xs px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-500 dark:text-blue-400">
                {t('workflow.strategy.modelBadge', { model: data.model })}
              </span>
            )}
            <span
              className={clsx(
                'text-xs px-2 py-0.5 rounded-full font-medium',
                confidencePct >= 80
                  ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                  : confidencePct >= 50
                    ? 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300'
                    : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
              )}
            >
              {t('workflow.strategy.confidence', { pct: confidencePct })}
            </span>
          </div>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Strategy name */}
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-indigo-800 dark:text-indigo-200">{localizedStrategy}</span>
        </div>

        {/* Reasoning */}
        <p className="text-xs text-indigo-700/80 dark:text-indigo-300/80">{data.reasoning}</p>

        {/* Dimension bars */}
        <div className="grid grid-cols-3 gap-2">
          <DimensionPill label={t('workflow.strategy.risk')} value={localizedRisk} color={riskColor(data.riskLevel)} />
          <DimensionPill
            label={t('workflow.strategy.stories')}
            value={String(data.estimatedStories)}
            color="text-indigo-600 dark:text-indigo-400"
          />
          <DimensionPill
            label={t('workflow.strategy.parallel')}
            value={localizedParallel}
            color={benefitColor(data.parallelizationBenefit)}
          />
        </div>

        {/* Functional areas */}
        {data.functionalAreas.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {data.functionalAreas.map((area) => (
              <span
                key={area}
                className="text-2xs px-1.5 py-0.5 rounded bg-indigo-100 dark:bg-indigo-900/40 text-indigo-600 dark:text-indigo-400"
              >
                {area}
              </span>
            ))}
          </div>
        )}

        {/* Recommendations */}
        {data.recommendations.length > 0 && (
          <div className="space-y-1">
            {data.recommendations.map((rec, i) => (
              <p key={i} className="text-2xs text-indigo-600/70 dark:text-indigo-400/70 flex items-start gap-1">
                <span className="shrink-0 mt-0.5">•</span>
                <span>{rec}</span>
              </p>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function localizeStrategyValue(t: ReturnType<typeof useTranslation>['t'], strategy: string): string {
  const key = `workflow.strategy.values.strategy.${strategy}`;
  const localized = t(key);
  return localized === key ? strategy.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()) : localized;
}

function localizeStrategyRisk(t: ReturnType<typeof useTranslation>['t'], level: string): string {
  const key = `workflow.strategy.values.risk.${level}`;
  const localized = t(key);
  return localized === key ? level : localized;
}

function localizeStrategyParallel(t: ReturnType<typeof useTranslation>['t'], benefit: string): string {
  const key = `workflow.strategy.values.parallel.${benefit}`;
  const localized = t(key);
  return localized === key ? benefit : localized;
}

function DimensionPill({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div className="text-center">
      <span className="text-2xs text-gray-500 dark:text-gray-400 block">{label}</span>
      <span className={clsx('text-xs font-medium', color)}>{value}</span>
    </div>
  );
}

function riskColor(level: string): string {
  switch (level) {
    case 'low':
      return 'text-green-600 dark:text-green-400';
    case 'medium':
      return 'text-amber-600 dark:text-amber-400';
    case 'high':
      return 'text-red-600 dark:text-red-400';
    default:
      return 'text-gray-600 dark:text-gray-400';
  }
}

function benefitColor(benefit: string): string {
  switch (benefit) {
    case 'significant':
      return 'text-green-600 dark:text-green-400';
    case 'moderate':
      return 'text-amber-600 dark:text-amber-400';
    case 'none':
      return 'text-gray-500 dark:text-gray-400';
    default:
      return 'text-gray-600 dark:text-gray-400';
  }
}
