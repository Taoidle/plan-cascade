/**
 * DesignDocCard
 *
 * Displays a non-interactive summary of the generated design document,
 * showing components, patterns, decisions, and feature mappings.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronRightIcon } from '@radix-ui/react-icons';
import type { DesignDocCardData } from '../../../types/workflowCard';
import { Collapsible } from '../Collapsible';

export function DesignDocCard({ data }: { data: DesignDocCardData }) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="rounded-lg border border-teal-200 dark:border-teal-800 bg-teal-50 dark:bg-teal-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-teal-100/50 dark:bg-teal-900/30 border-b border-teal-200 dark:border-teal-800">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-teal-700 dark:text-teal-300 uppercase tracking-wide">
            {t('workflow.designDoc.title')}
          </span>
          <button
            onClick={() => setExpanded((v) => !v)}
            className="text-2xs text-teal-600 dark:text-teal-400 hover:text-teal-800 dark:hover:text-teal-200 transition-colors"
          >
            <ChevronRightIcon
              className={clsx('w-3.5 h-3.5 transition-transform duration-200', expanded && 'rotate-90')}
            />
          </button>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Title & summary */}
        <p className="text-sm font-medium text-teal-800 dark:text-teal-200">{data.title}</p>
        <p className="text-xs text-teal-700/80 dark:text-teal-300/80">{data.summary}</p>

        {/* Stats row */}
        <div className="grid grid-cols-4 gap-2">
          <StatPill label={t('workflow.designDoc.components')} value={data.componentsCount} />
          <StatPill label={t('workflow.designDoc.patterns')} value={data.patternsCount} />
          <StatPill label={t('workflow.designDoc.decisions')} value={data.decisionsCount} />
          <StatPill label={t('workflow.designDoc.featureMappings')} value={data.featureMappingsCount} />
        </div>

        {/* Expanded details */}
        <Collapsible open={expanded}>
          <div className="space-y-2 pt-1 border-t border-teal-200 dark:border-teal-800">
            {/* Component names */}
            {data.componentNames.length > 0 && (
              <div>
                <span className="text-2xs font-medium text-teal-600 dark:text-teal-400">
                  {t('workflow.designDoc.components')}
                </span>
                <div className="flex flex-wrap gap-1 mt-0.5">
                  {data.componentNames.map((name) => (
                    <span
                      key={name}
                      className="text-2xs px-1.5 py-0.5 rounded bg-teal-100 dark:bg-teal-900/40 text-teal-600 dark:text-teal-400"
                    >
                      {name}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* Pattern names */}
            {data.patternNames.length > 0 && (
              <div>
                <span className="text-2xs font-medium text-teal-600 dark:text-teal-400">
                  {t('workflow.designDoc.patterns')}
                </span>
                <div className="flex flex-wrap gap-1 mt-0.5">
                  {data.patternNames.map((name) => (
                    <span
                      key={name}
                      className="text-2xs px-1.5 py-0.5 rounded bg-teal-100 dark:bg-teal-900/40 text-teal-600 dark:text-teal-400"
                    >
                      {name}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* Saved path */}
            {data.savedPath && (
              <p className="text-2xs text-teal-500 dark:text-teal-400/70">
                {t('workflow.designDoc.savedTo', { path: data.savedPath })}
              </p>
            )}
          </div>
        </Collapsible>
      </div>
    </div>
  );
}

function StatPill({ label, value }: { label: string; value: number }) {
  return (
    <div className="text-center">
      <span className="text-2xs text-gray-500 dark:text-gray-400 block">{label}</span>
      <span className={clsx('text-xs font-medium text-teal-600 dark:text-teal-400')}>{value}</span>
    </div>
  );
}
