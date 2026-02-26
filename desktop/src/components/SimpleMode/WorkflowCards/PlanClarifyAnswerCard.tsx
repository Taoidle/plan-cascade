/**
 * Plan Clarify Answer Card
 *
 * Displays a user's clarification answer.
 */

import { useTranslation } from 'react-i18next';
import type { PlanClarifyAnswerCardData } from '../../../types/planModeCard';

export function PlanClarifyAnswerCard({ data }: { data: PlanClarifyAnswerCardData }) {
  const { t } = useTranslation('planMode');

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/20 px-3 py-2">
      <div className="flex items-center gap-2">
        <span className="text-2xs text-gray-400">#{data.questionId}</span>
        {data.skipped ? (
          <span className="text-2xs text-gray-500 italic">{t('clarify.skipped', 'Skipped')}</span>
        ) : (
          <span className="text-xs text-gray-700 dark:text-gray-300">{data.answer}</span>
        )}
      </div>
    </div>
  );
}
