/**
 * InterviewQuestionCard
 *
 * Displays an interview question in the chat. Read-only after answered.
 * Structured input (boolean/select) is handled by StructuredInputOverlay.
 */

import { clsx } from 'clsx';
import type { InterviewQuestionCardData } from '../../../types/workflowCard';

export function InterviewQuestionCard({ data }: { data: InterviewQuestionCardData }) {
  return (
    <div className="rounded-lg border border-violet-200 dark:border-violet-800 bg-violet-50 dark:bg-violet-900/20 px-3 py-2">
      <div className="flex items-center justify-between">
        <span className="text-2xs font-medium text-violet-600 dark:text-violet-400 uppercase tracking-wide">
          Interview Question
        </span>
        <span className="text-2xs text-violet-500 dark:text-violet-400">
          {data.questionNumber}/{data.totalQuestions}
        </span>
      </div>

      <p className="text-sm text-violet-800 dark:text-violet-200 mt-1 font-medium">
        {data.question}
      </p>

      {data.hint && (
        <p className="text-xs text-violet-600/70 dark:text-violet-400/70 mt-1 italic">
          {data.hint}
        </p>
      )}

      <div className="mt-1 flex items-center gap-2">
        <span
          className={clsx(
            'text-2xs px-1.5 py-0.5 rounded',
            data.required
              ? 'bg-violet-200 dark:bg-violet-800 text-violet-600 dark:text-violet-400'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400'
          )}
        >
          {data.required ? 'required' : 'optional'}
        </span>
        <span className="text-2xs text-violet-500/60 dark:text-violet-400/60">
          {formatInputType(data.inputType)}
        </span>
      </div>
    </div>
  );
}

function formatInputType(type: InterviewQuestionCardData['inputType']): string {
  switch (type) {
    case 'text': return 'text input';
    case 'textarea': return 'text area';
    case 'single_select': return 'choose one';
    case 'multi_select': return 'choose multiple';
    case 'boolean': return 'yes / no';
    default: return type;
  }
}
