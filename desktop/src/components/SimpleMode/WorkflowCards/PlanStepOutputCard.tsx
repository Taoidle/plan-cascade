/**
 * Plan Step Output Card
 *
 * Displays the output produced by a completed step.
 * Shows content with criterion validation results.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon } from '@radix-ui/react-icons';
import type { PlanStepOutputCardData } from '../../../types/planModeCard';

export function PlanStepOutputCard({ data }: { data: PlanStepOutputCardData }) {
  const { t } = useTranslation('planMode');
  const [isExpanded, setIsExpanded] = useState(false);
  const allMet = data.criteriaMet.length === 0 || data.criteriaMet.every((c) => c.met);

  return (
    <div
      className={clsx(
        'rounded-lg border px-3 py-2',
        allMet
          ? 'border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20'
          : 'border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20',
      )}
    >
      {/* Header */}
      <button onClick={() => setIsExpanded(!isExpanded)} className="w-full flex items-center gap-2 text-left">
        {isExpanded ? (
          <ChevronDownIcon className="w-3 h-3 shrink-0 text-gray-400" />
        ) : (
          <ChevronRightIcon className="w-3 h-3 shrink-0 text-gray-400" />
        )}
        <span className="text-xs font-medium text-gray-800 dark:text-gray-200 flex-1">{data.stepTitle}</span>
        <span
          className={clsx(
            'text-2xs px-1.5 py-0.5 rounded font-medium',
            allMet
              ? 'bg-green-100 dark:bg-green-900/40 text-green-600 dark:text-green-400'
              : 'bg-amber-100 dark:bg-amber-900/40 text-amber-600 dark:text-amber-400',
          )}
        >
          {allMet ? t('output.passed', 'passed') : t('output.partial', 'partial')}
        </span>
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div className="mt-2 space-y-2">
          {/* Output content */}
          <div className="text-xs text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-900 rounded p-2 max-h-48 overflow-y-auto whitespace-pre-wrap font-mono">
            {data.content.length > 1000 ? `${data.content.slice(0, 1000)}...` : data.content}
          </div>

          {/* Criteria results */}
          {data.criteriaMet.length > 0 && (
            <div className="space-y-0.5">
              <span className="text-2xs font-medium text-gray-500">{t('output.criteria', 'Criteria')}:</span>
              {data.criteriaMet.map((cr, i) => (
                <div key={i} className="flex items-start gap-1.5 text-2xs">
                  <span className={cr.met ? 'text-green-500' : 'text-red-500'}>{cr.met ? '\u2713' : '\u2717'}</span>
                  <span className="text-gray-600 dark:text-gray-400">
                    {cr.criterion}: {cr.explanation}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
