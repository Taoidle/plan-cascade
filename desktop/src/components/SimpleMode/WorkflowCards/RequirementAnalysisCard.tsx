/**
 * RequirementAnalysisCard
 *
 * Displays the ProductManager persona's requirement analysis results.
 * Shows key requirements, identified gaps, and suggested scope.
 * Uses amber color scheme. Non-interactive, expandable.
 */

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { RequirementAnalysisCardData } from '../../../types/workflowCard';

export function RequirementAnalysisCard({ data }: { data: RequirementAnalysisCardData }) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-amber-100/50 dark:bg-amber-900/30 border-b border-amber-200 dark:border-amber-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-amber-700 dark:text-amber-300 uppercase tracking-wide">
              {t('workflow.requirementAnalysis.title')}
            </span>
            <span className="text-2xs px-1.5 py-0.5 rounded bg-amber-200 dark:bg-amber-800 text-amber-600 dark:text-amber-400">
              {data.personaRole}
            </span>
          </div>
          <button
            onClick={() => setExpanded((v) => !v)}
            className="text-2xs text-amber-600 dark:text-amber-400 hover:text-amber-800 dark:hover:text-amber-200 transition-colors"
          >
            {expanded ? '▲' : '▼'}
          </button>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Key Requirements */}
        {data.keyRequirements.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-amber-600 dark:text-amber-400">
              {t('workflow.requirementAnalysis.keyRequirements')}
            </span>
            <ul className="mt-0.5 space-y-0.5">
              {data.keyRequirements.slice(0, expanded ? undefined : 3).map((req, i) => (
                <li key={i} className="text-2xs text-amber-700 dark:text-amber-300 flex items-start gap-1">
                  <span className="text-amber-400 dark:text-amber-500 shrink-0 mt-px">•</span>
                  <span>{req}</span>
                </li>
              ))}
              {!expanded && data.keyRequirements.length > 3 && (
                <li className="text-2xs text-amber-500 dark:text-amber-400 italic">
                  +{data.keyRequirements.length - 3} more...
                </li>
              )}
            </ul>
          </div>
        )}

        {/* Identified Gaps */}
        {data.identifiedGaps.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-amber-600 dark:text-amber-400">
              {t('workflow.requirementAnalysis.gaps')}
            </span>
            <ul className="mt-0.5 space-y-0.5">
              {data.identifiedGaps.slice(0, expanded ? undefined : 2).map((gap, i) => (
                <li key={i} className="text-2xs text-amber-600 dark:text-amber-400 flex items-start gap-1">
                  <span className="text-amber-400 dark:text-amber-500 shrink-0 mt-px">⚠</span>
                  <span>{gap}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Suggested Scope */}
        <div>
          <span className="text-2xs font-medium text-amber-600 dark:text-amber-400">
            {t('workflow.requirementAnalysis.scope')}
          </span>
          <p className="mt-0.5 text-2xs text-amber-700/80 dark:text-amber-300/80">{data.suggestedScope}</p>
        </div>

        {/* Expanded: Full Analysis */}
        {expanded && data.analysis && (
          <div className="pt-1 border-t border-amber-200 dark:border-amber-800">
            <span className="text-2xs font-medium text-amber-600 dark:text-amber-400">
              {t('workflow.requirementAnalysis.fullAnalysis', 'Full Analysis')}
            </span>
            <div className="mt-0.5 text-2xs text-amber-700/80 dark:text-amber-300/80 whitespace-pre-wrap">
              {data.analysis}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
