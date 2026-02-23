/**
 * InterviewAnswerCard
 *
 * Displays a user's interview answer (right-aligned bubble style).
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { InterviewAnswerCardData } from '../../../types/workflowCard';

export function InterviewAnswerCard({ data }: { data: InterviewAnswerCardData }) {
  const { t } = useTranslation('simpleMode');
  return (
    <div className="flex justify-end">
      <div
        className={clsx(
          'max-w-[82%] px-3 py-2 rounded-2xl rounded-br-sm text-sm',
          data.skipped
            ? 'bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400 italic'
            : 'bg-violet-600 text-white'
        )}
      >
        {data.skipped ? t('workflow.interview.skipped') : data.answer}
      </div>
    </div>
  );
}
