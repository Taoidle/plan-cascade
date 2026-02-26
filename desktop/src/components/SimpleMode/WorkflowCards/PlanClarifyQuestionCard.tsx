/**
 * Plan Clarify Question Card
 *
 * Displays a clarification question during the clarifying phase.
 */

import { useTranslation } from 'react-i18next';
import type { PlanClarifyQuestionCardData } from '../../../types/planModeCard';

export function PlanClarifyQuestionCard({ data }: { data: PlanClarifyQuestionCardData }) {
  const { t } = useTranslation('planMode');

  return (
    <div className="rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 px-3 py-2">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-sm">&#x2753;</span>
        <span className="text-xs font-semibold text-amber-700 dark:text-amber-300">
          {t('clarify.question', 'Clarification Question')}
        </span>
        <span className="text-2xs text-amber-500">#{data.questionId}</span>
      </div>
      <p className="text-xs text-gray-700 dark:text-gray-300">{data.question}</p>
      {data.hint && (
        <p className="mt-1 text-2xs text-gray-500 dark:text-gray-400 italic">
          {t('clarify.hint', 'Hint')}: {data.hint}
        </p>
      )}
    </div>
  );
}
