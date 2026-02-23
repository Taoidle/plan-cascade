/**
 * InterviewAnswerCard
 *
 * Displays a user's interview answer (right-aligned bubble style).
 */

import { clsx } from 'clsx';
import type { InterviewAnswerCardData } from '../../../types/workflowCard';

export function InterviewAnswerCard({ data }: { data: InterviewAnswerCardData }) {
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
        {data.skipped ? 'Skipped' : data.answer}
      </div>
    </div>
  );
}
