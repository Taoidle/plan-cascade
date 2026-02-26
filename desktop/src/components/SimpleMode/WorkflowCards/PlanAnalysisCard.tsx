/**
 * Plan Analysis Card
 *
 * Displays the domain analysis result from the analyzing phase.
 * Shows detected domain, complexity, suggested approach, and adapter.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { PlanAnalysisCardData } from '../../../types/planModeCard';

function complexityColor(complexity: number): string {
  if (complexity <= 3) return 'text-green-600 dark:text-green-400 bg-green-100 dark:bg-green-900/40';
  if (complexity <= 6) return 'text-amber-600 dark:text-amber-400 bg-amber-100 dark:bg-amber-900/40';
  return 'text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-900/40';
}

function domainLabel(domain: string): string {
  const labels: Record<string, string> = {
    general: 'General',
    writing: 'Writing',
    research: 'Research',
    marketing: 'Marketing',
    data_analysis: 'Data Analysis',
    project_management: 'Project Management',
  };
  return labels[domain] || domain;
}

export function PlanAnalysisCard({ data }: { data: PlanAnalysisCardData }) {
  const { t } = useTranslation('planMode');

  return (
    <div className="rounded-lg border border-violet-200 dark:border-violet-800 bg-violet-50 dark:bg-violet-900/20">
      {/* Header */}
      <div className="px-3 py-2 bg-violet-100/50 dark:bg-violet-900/30 border-b border-violet-200 dark:border-violet-800 flex items-center justify-between">
        <span className="text-xs font-semibold text-violet-700 dark:text-violet-300">
          {t('analysis.title', 'Plan Analysis')}
        </span>
        <div className="flex items-center gap-2">
          <span className={clsx('text-2xs px-1.5 py-0.5 rounded font-medium', complexityColor(data.complexity))}>
            {t('analysis.complexity', 'Complexity')}: {data.complexity}/10
          </span>
        </div>
      </div>

      {/* Content */}
      <div className="px-3 py-2 space-y-2">
        {/* Domain & Adapter */}
        <div className="flex items-center gap-3">
          <span className="text-2xs px-1.5 py-0.5 rounded bg-violet-100 dark:bg-violet-900/40 text-violet-600 dark:text-violet-400 font-medium">
            {domainLabel(data.domain)}
          </span>
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            {t('analysis.adapter', 'Adapter')}: {data.adapterName}
          </span>
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            ~{data.estimatedSteps} {t('analysis.steps', 'steps')}
          </span>
        </div>

        {/* Reasoning */}
        <p className="text-xs text-gray-700 dark:text-gray-300">{data.reasoning}</p>

        {/* Suggested Approach */}
        <div className="text-xs text-gray-600 dark:text-gray-400">
          <span className="font-medium">{t('analysis.approach', 'Approach')}:</span> {data.suggestedApproach}
        </div>

        {/* Clarification indicator */}
        {data.needsClarification && (
          <div className="text-2xs text-amber-600 dark:text-amber-400 flex items-center gap-1">
            <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
              <path
                fillRule="evenodd"
                d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
                clipRule="evenodd"
              />
            </svg>
            {t('analysis.needsClarification', 'Some details need clarification')}
          </div>
        )}
      </div>
    </div>
  );
}
